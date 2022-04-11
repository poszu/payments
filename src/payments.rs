use itertools::Itertools;
use std::collections::HashMap;

use crate::{
    client::{Client, ClientId},
    error::Error,
    transaction::Transaction,
};

#[derive(Debug, Default)]
pub struct Payments {
    clients: HashMap<ClientId, Client>,
}

impl Payments {
    /// Apply a transaction
    pub fn apply(&mut self, transaction: Transaction) -> Result<(), Error> {
        let client = self
            .clients
            .entry(transaction.client_id)
            .or_insert_with(|| Client::new(transaction.client_id));

        // TODO: what if:
        // The client has just been inserted (it's a new one) AND
        // the operation failed.
        client.apply(transaction.op)
    }

    /// Serialize the payments' client database to CSV
    /// Note: sorts clients by ID for predicatable output (for testing purposes).
    /// I assumed, that serialization is rare and it's OK to slow down a bit to have
    /// a consistent outcome.
    pub fn serialize(&self, output: impl std::io::Write) -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = csv::Writer::from_writer(output);
        for client in self.clients.values().sorted_by_key(|c| c.id) {
            writer.serialize(client)?
        }
        writer.flush()?;
        Ok(())
    }
}
