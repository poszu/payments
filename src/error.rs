use rust_decimal::Decimal;
use thiserror::Error;

use crate::{client::OperationState, transaction::TransactionId};

#[derive(Error, Debug, PartialEq)]
pub enum Error {
    #[error("failed to parse input, reason: `{0}`")]
    ParsingFailure(String),
    #[error("transaction ID `{0}` (for Deposit/Withdrawal) is duplicated")]
    DuplicatedTransaction(TransactionId),
    #[error("transaction ID `{0}` (for Dispute/Resolve/ChargeBack) not found")]
    TransactionNotFound(TransactionId),
    #[error("withdrawal transaction ID `{id:?}` of {requested:?} failed because of insufficient funds: {available:?}")]
    InsufficientFunds {
        id: TransactionId,
        available: Decimal,
        requested: Decimal,
    },
    #[error("invalid transaction state transition for ID `{id:?}` ({from:?} -> {to:?})")]
    InvalidTransactionStateChange {
        id: TransactionId,
        from: OperationState,
        to: OperationState,
    },
    #[error("transaction ID `{0}` was tried on a locked account")]
    AccountLocked(TransactionId),

    #[error(
        "failed to dispute transaction ID `{0}` as it would result in negative account balance"
    )]
    FailedDisputeNotEnoughFunds(TransactionId),
}
