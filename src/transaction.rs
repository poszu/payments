use rust_decimal::Decimal;

use crate::client::ClientId;

pub type TransactionId = u32;

#[derive(Debug, PartialEq)]
pub enum OperationType {
    Deposit { amount: Decimal },
    Withdrawal { amount: Decimal },
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, PartialEq)]
pub struct Operation {
    pub id: TransactionId,
    pub kind: OperationType,
}

#[derive(Debug, PartialEq)]
pub struct Transaction {
    pub op: Operation,
    pub client_id: ClientId,
}
