use std::collections::{hash_map::Entry, HashMap};

use rust_decimal::Decimal;
use serde::Serialize;

use crate::{
    error::Error,
    transaction::{Operation, OperationType, TransactionId},
};

/// Represents possible states of an operation,
/// along with all allowed transitions.
/// Allowed state transitions:
/// New -> InDispute
/// InDispute -> Resolved | Chargedback
/// Assumption: it is not possible to dispute a given transaction twice,
/// hence there is no `Resolved -> InDispute` state transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperationState {
    New,
    InDispute,
    Resolved,
    Chargedback,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct StatefulOperation {
    id: TransactionId,
    amount: Decimal,
    state: OperationState,
}

impl StatefulOperation {
    fn new(id: TransactionId, amount: Decimal) -> Self {
        StatefulOperation {
            id,
            amount,
            state: OperationState::New,
        }
    }

    fn state_transition(&mut self, new_state: OperationState) -> Result<(), Error> {
        self.state = match (self.state, new_state) {
            (OperationState::New, OperationState::InDispute) => Ok(new_state),
            (OperationState::InDispute, OperationState::Resolved) => Ok(new_state),
            (OperationState::InDispute, OperationState::Chargedback) => Ok(new_state),
            (from, to) if from == to => Ok(from),
            (from, to) => Err(Error::InvalidTransactionStateChange {
                id: self.id,
                from,
                to,
            }),
        }?;
        Ok(())
    }
}

pub type ClientId = u16;

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Client {
    #[serde(rename = "client")]
    pub id: ClientId,
    #[serde(skip_serializing)]
    // Assumption: it is not required to keep track of the order of transactions,
    // hence using a hashmap here
    operations: HashMap<TransactionId, StatefulOperation>,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

impl Client {
    pub fn new(id: ClientId) -> Self {
        Self {
            id,
            ..Self::default()
        }
    }

    fn try_deposit(&mut self, id: TransactionId, amount: Decimal) -> Result<(), Error> {
        if self.operations.contains_key(&id) {
            return Err(Error::DuplicatedTransaction(id));
        }
        self.operations
            .insert(id, StatefulOperation::new(id, amount));
        self.total += amount;
        self.available += amount;
        Ok(())
    }

    fn try_withdraw(&mut self, id: TransactionId, amount: Decimal) -> Result<(), Error> {
        if self.operations.contains_key(&id) {
            return Err(Error::DuplicatedTransaction(id));
        }
        if self.available < amount {
            return Err(Error::InsufficientFunds {
                id,
                available: self.available,
                requested: amount,
            });
        }
        self.operations
            .insert(id, StatefulOperation::new(id, -amount));
        self.total -= amount;
        self.available -= amount;
        Ok(())
    }

    /// A dispute represents a client's claim that a transaction was erroneous and should be reversed.
    /// The transaction shouldn't be reversed yet but the associated funds should be held. This means
    /// that the clients available funds should decrease by the amount disputed, their held funds should
    /// increase by the amount disputed, while their total funds should remain the same.
    fn try_dispute(&mut self, id: TransactionId) -> Result<(), Error> {
        if let Entry::Occupied(mut op) = self.operations.entry(id) {
            let op = op.get_mut();
            if self.available < op.amount {
                return Err(Error::FailedDisputeNotEnoughFunds(id));
            }

            op.state_transition(OperationState::InDispute)?;
            self.available -= op.amount;
            self.held += op.amount;
            Ok(())
        } else {
            Err(Error::TransactionNotFound(id))
        }
    }

    /// A resolve represents a resolution to a dispute, releasing the associated held funds. Funds that
    /// were previously disputed are no longer disputed. This means that the clients held funds should
    /// decrease by the amount no longer disputed, their available funds should increase by the
    /// amount no longer disputed, and their total funds should remain the same.
    fn try_resolve(&mut self, id: TransactionId) -> Result<(), Error> {
        if let Entry::Occupied(mut op) = self.operations.entry(id) {
            let op = op.get_mut();
            op.state_transition(OperationState::Resolved)?;
            self.available += op.amount;
            self.held -= op.amount;
            Ok(())
        } else {
            Err(Error::TransactionNotFound(id))
        }
    }

    /// A chargeback is the final state of a dispute and represents the client reversing a transaction.
    /// Funds that were held have now been withdrawn. This means that the clients held funds and
    /// total funds should decrease by the amount previously disputed. If a chargeback occurs the
    /// client's account should be immediately frozen.
    fn try_chargeback(&mut self, id: TransactionId) -> Result<(), Error> {
        if let Entry::Occupied(mut op) = self.operations.entry(id) {
            let op = op.get_mut();
            op.state_transition(OperationState::Chargedback)?;
            self.held -= op.amount;
            self.total -= op.amount;
            self.locked = true;
            Ok(())
        } else {
            Err(Error::TransactionNotFound(id))
        }
    }

    pub fn apply(&mut self, op: Operation) -> Result<(), Error> {
        if self.locked {
            return Err(Error::AccountLocked(op.id));
        }
        match op.kind {
            OperationType::Deposit { amount } => self.try_deposit(op.id, amount),
            OperationType::Withdrawal { amount } => self.try_withdraw(op.id, amount),
            OperationType::Dispute => self.try_dispute(op.id),
            OperationType::Resolve => self.try_resolve(op.id),
            OperationType::Chargeback => self.try_chargeback(op.id),
        }
    }
}

#[cfg(test)]
mod test {

    /// Test all possible Operation state changes
    mod operation_state_changes {
        use crate::client::{OperationState, StatefulOperation};
        use crate::error::Error;
        use rust_decimal_macros::dec;

        macro_rules! test_allowed_operation_state_changes {
            ($(OperationState::$from:ident => OperationState::$to:ident,)*) => {
            $(
                paste::paste! {
                #[test]
                fn [<$from:lower _to_  $to:lower>]() {
                    assert_eq!(
                        Ok(()),
                        StatefulOperation {
                            id: 0,
                            amount: dec!(0),
                            state: OperationState::$from,
                        }
                        .state_transition(OperationState::$to)
                    );
                }
            }
            )*
            }
        }

        macro_rules! test_disallowed_operation_state_changes {
            ($(OperationState::$from:ident => OperationState::$to:ident,)*) => {
            $(
                paste::paste! {
                #[test]
                fn [<$from:lower _to_  $to:lower>]() {
                    assert_eq!(
                        Err(Error::InvalidTransactionStateChange { id: 0, from: OperationState::$from, to: OperationState::$to }),
                        StatefulOperation {
                            id: 0,
                            amount: dec!(0),
                            state: OperationState::$from,
                        }
                        .state_transition(OperationState::$to)
                    );
                }
            }
            )*
            }
        }

        test_allowed_operation_state_changes! {
            OperationState::New => OperationState::InDispute,
            OperationState::InDispute => OperationState::Resolved,
            OperationState::InDispute => OperationState::Chargedback,
            OperationState::New => OperationState::New,
            OperationState::InDispute => OperationState::InDispute,
            OperationState::Resolved => OperationState::Resolved,
            OperationState::Chargedback => OperationState::Chargedback,
        }

        test_disallowed_operation_state_changes! {
            OperationState::New => OperationState::Resolved,
            OperationState::New => OperationState::Chargedback,
            OperationState::InDispute => OperationState::New,
            OperationState::Chargedback => OperationState::New,
            OperationState::Chargedback => OperationState::InDispute,
            OperationState::Chargedback => OperationState::Resolved,
            OperationState::Resolved => OperationState::New,
            OperationState::Resolved => OperationState::InDispute,
            OperationState::Resolved => OperationState::Chargedback,
        }
    }
    mod applying_transactions {
        use crate::{
            client::Client,
            error::Error,
            transaction::{Operation, OperationType},
        };
        use rust_decimal_macros::dec;

        macro_rules! check_balance {
            ($cl:ident has available:$available:literal held:$held:literal total:$total:literal) => {
                assert_eq!(
                    (dec!($available), dec!($held), dec!($total)),
                    ($cl.available, $cl.held, $cl.total)
                );
            };
        }
        #[test]
        fn new_deposit() {
            let mut client = Client::new(0);
            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Deposit { amount: dec!(1.25) }
                })
            );
            check_balance!(client has available:1.25 held:0 total:1.25);
            assert!(!client.locked);
        }

        #[test]
        fn duplicated_deposit_id() {
            let mut client = Client::new(0);
            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Deposit { amount: dec!(1.25) }
                })
            );
            assert_eq!(
                Err(Error::DuplicatedTransaction(0)),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Deposit { amount: dec!(1.25) }
                })
            );
            check_balance!(client has available:1.25 held:0 total:1.25);
            assert!(!client.locked);
        }

        #[test]
        fn dispute() {
            let mut client = Client::new(0);
            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Deposit { amount: dec!(1.25) }
                })
            );
            check_balance!(client has available:1.25 held:0 total:1.25);
            assert!(!client.locked);

            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Dispute
                })
            );
            check_balance!(client has available:0 held:1.25 total:1.25);
            assert!(!client.locked);
        }

        #[test]
        fn dispute_below_balance() {
            let mut client = Client::new(0);
            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Deposit { amount: dec!(1) }
                })
            );
            check_balance!(client has available:1 held:0 total:1);

            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 1,
                    kind: OperationType::Withdrawal { amount: dec!(1) }
                })
            );
            check_balance!(client has available:0 held:0 total:0);

            assert_eq!(
                Err(Error::FailedDisputeNotEnoughFunds(0)),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Dispute
                })
            );
            check_balance!(client has available:0 held:0 total:0);
            assert!(!client.locked);
        }

        #[test]
        fn resolve() {
            let mut client = Client::new(0);
            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Deposit { amount: dec!(1.25) }
                })
            );
            check_balance!(client has available:1.25 held:0 total:1.25);
            assert!(!client.locked);

            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Dispute
                })
            );
            check_balance!(client has available:0 held:1.25 total:1.25);
            assert!(!client.locked);

            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Resolve
                })
            );
            check_balance!(client has available:1.25 held:0 total:1.25);
            assert!(!client.locked);
        }

        #[test]
        fn chargeback() {
            let mut client = Client::new(0);
            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Deposit { amount: dec!(1.25) }
                })
            );
            check_balance!(client has available:1.25 held:0 total:1.25);

            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Dispute
                })
            );
            check_balance!(client has available:0 held:1.25 total:1.25);

            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Chargeback
                })
            );
            check_balance!(client has available:0 held:0 total:0);

            // Account is now locked (frozen)
            assert!(client.locked);
            assert_eq!(
                client.apply(Operation {
                    id: 1,
                    kind: OperationType::Deposit { amount: dec!(1) }
                }),
                Err(Error::AccountLocked(1))
            );
            check_balance!(client has available:0 held:0 total:0);
        }

        #[test]
        fn withdraw() {
            let mut client = Client::new(0);
            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Deposit { amount: dec!(1.25) }
                })
            );
            check_balance!(client has available:1.25 held:0 total:1.25);

            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 1,
                    kind: OperationType::Withdrawal { amount: dec!(.25) }
                })
            );
            check_balance!(client has available:1 held:0 total:1);
            assert!(!client.locked);
        }

        #[test]
        fn cannot_withdraw_below_balance() {
            let mut client = Client::new(0);
            assert_eq!(
                Err(Error::InsufficientFunds {
                    id: 0,
                    available: dec!(0),
                    requested: dec!(1)
                }),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Withdrawal { amount: dec!(1) }
                })
            );
            check_balance!(client has available:0 held:0 total:0);
            assert!(!client.locked);
        }

        #[test]
        fn cannot_withdraw_below_balance_non_zero() {
            let mut client = Client::new(0);
            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Deposit { amount: dec!(1) }
                })
            );
            check_balance!(client has available:1 held:0 total:1);

            assert_eq!(
                Err(Error::InsufficientFunds {
                    id: 1,
                    available: dec!(1),
                    requested: dec!(2)
                }),
                client.apply(Operation {
                    id: 1,
                    kind: OperationType::Withdrawal { amount: dec!(2) }
                })
            );
            check_balance!(client has available:1 held:0 total:1);
        }

        #[test]
        fn cannot_withdraw_held() {
            let mut client = Client::new(0);
            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Deposit { amount: dec!(1) }
                })
            );
            check_balance!(client has available:1 held:0 total:1);

            assert_eq!(
                Ok(()),
                client.apply(Operation {
                    id: 0,
                    kind: OperationType::Dispute
                })
            );
            check_balance!(client has available:0 held:1 total:1);

            assert_eq!(
                Err(Error::InsufficientFunds {
                    id: 2,
                    available: dec!(0),
                    requested: dec!(1)
                }),
                client.apply(Operation {
                    id: 2,
                    kind: OperationType::Withdrawal { amount: dec!(1) }
                })
            );
            check_balance!(client has available:0 held:1 total:1);
        }
    }
}
