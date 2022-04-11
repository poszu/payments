use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    error::Error,
    transaction::{Operation, OperationType, Transaction},
};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ParsedTransactionKind {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Deserialize, Debug, PartialEq)]
struct ParsedTransaction {
    #[serde(rename = "type")]
    kind: ParsedTransactionKind,
    client: u16,
    tx: u32,
    amount: Option<Decimal>,
}

pub fn parse<R>(rdr: csv::Reader<R>) -> impl Iterator<Item = Result<Transaction, Error>>
where
    R: std::io::Read,
{
    rdr.into_deserialize::<ParsedTransaction>().map(|trans| {
        let trans = trans.map_err(|e| Error::ParsingFailure(e.to_string()))?;

        // The intermediate representation is required as `csv` crate doesn't
        // support serde's internally tagged enums.
        // We want to guarantee on a type-level that Deposit and Withdrawal have amounts specified.
        Ok(Transaction {
            client_id: trans.client,
            op: Operation {
                id: trans.tx,
                kind: match trans.kind {
                    ParsedTransactionKind::Deposit => OperationType::Deposit {
                        amount: trans.amount.ok_or_else(|| {
                            Error::ParsingFailure(
                                "deposit transaction must have amount".to_string(),
                            )
                        })?,
                    },
                    ParsedTransactionKind::Withdrawal => OperationType::Withdrawal {
                        amount: trans.amount.ok_or_else(|| {
                            Error::ParsingFailure(
                                "withdrawal transaction must have amount".to_string(),
                            )
                        })?,
                    },
                    ParsedTransactionKind::Dispute => OperationType::Dispute,
                    ParsedTransactionKind::Resolve => OperationType::Resolve,
                    ParsedTransactionKind::Chargeback => OperationType::Chargeback,
                },
            },
        })
    })
}

#[cfg(test)]
mod tests {
    mod parsing {
        use rust_decimal_macros::dec;

        use crate::error::Error;
        use crate::parser::parse;
        use crate::transaction::{Operation, OperationType, Transaction};

        macro_rules! parse {
            ($data:literal) => {{
                let input = format!("type, client, tx, amount\n{}", $data);
                let rdr = csv::ReaderBuilder::new()
                    .trim(csv::Trim::All)
                    .from_reader(input.as_bytes());
                parse(rdr).collect::<Vec<Result<Transaction, _>>>()
            }};
        }

        #[test]
        fn parse_deposit() {
            assert_eq!(
                parse!("deposit, 1, 1, 1.0"),
                vec![Ok(Transaction {
                    client_id: 1,
                    op: Operation {
                        id: 1,
                        kind: OperationType::Deposit { amount: dec!(1.0) }
                    }
                })]
            );
            assert!(matches!(
                parse!("deposit, 1, 1,")[..],
                [Err(Error::ParsingFailure(_))]
            ));
        }
        #[test]
        fn parse_withdrawal() {
            assert_eq!(
                parse!("withdrawal, 1, 1, 1.0"),
                vec![Ok(Transaction {
                    client_id: 1,
                    op: Operation {
                        id: 1,
                        kind: OperationType::Withdrawal { amount: dec!(1.0) }
                    }
                })]
            );
            assert!(matches!(
                parse!("withdrawal, 1, 1,")[..],
                [Err(Error::ParsingFailure(_))]
            ));
        }
        #[test]
        fn parse_dispute() {
            assert_eq!(
                parse!("dispute, 1, 1,"),
                vec![Ok(Transaction {
                    client_id: 1,
                    op: Operation {
                        id: 1,
                        kind: OperationType::Dispute
                    }
                })]
            );
            assert_eq!(
                parse!("dispute, 1, 1, 1"),
                vec![Ok(Transaction {
                    client_id: 1,
                    op: Operation {
                        id: 1,
                        kind: OperationType::Dispute
                    }
                })]
            );
        }
        #[test]
        fn parse_resolve() {
            assert_eq!(
                parse!("resolve, 1, 1,"),
                vec![Ok(Transaction {
                    client_id: 1,
                    op: Operation {
                        id: 1,
                        kind: OperationType::Resolve
                    }
                })]
            );
        }
        #[test]
        fn parse_chargeback() {
            assert_eq!(
                parse!("chargeback, 1, 1,"),
                vec![Ok(Transaction {
                    client_id: 1,
                    op: Operation {
                        id: 1,
                        kind: OperationType::Chargeback
                    }
                })]
            );
        }
    }
}
