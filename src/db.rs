use std::{path::PathBuf, sync::Arc};

use anyhow::{Result, anyhow};
use redb::{Database, ReadableTable, TableDefinition};
use uuid::Uuid;

use crate::types::{QuoteInfo, QuoteState};

// <Y, QuoteInfo>
const QUOTES_TABLE: TableDefinition<&[u8], &str> = TableDefinition::new("quotes");

#[derive(Clone)]
pub struct Db {
    db: Arc<Database>,
}

impl Db {
    pub fn new(path: PathBuf) -> Result<Self> {
        let db = Database::create(path)?;

        let write_txn = db.begin_write()?;
        {
            // Open all tables to init a new db
            let _ = write_txn.open_table(QUOTES_TABLE)?;
        }

        write_txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }

    pub fn add_quote(&self, quote_info: &QuoteInfo) -> Result<()> {
        let write_txn = self.db.begin_write()?;

        {
            let mut quote_table = write_txn.open_table(QUOTES_TABLE)?;

            let _ = quote_table.insert(
                quote_info.id.into_bytes().as_slice(),
                serde_json::to_string(quote_info)?.as_str(),
            );
        }

        write_txn.commit()?;

        Ok(())
    }

    pub fn get_quote(&self, quote_id: Uuid) -> Result<QuoteInfo> {
        let read_txn = self.db.begin_read()?;

        let quote_table = read_txn.open_table(QUOTES_TABLE)?;
        let quote_value = quote_table
            .get(quote_id.into_bytes().as_slice())?
            .ok_or(anyhow!("Unknown quote"))?;

        let quote_value = quote_value.value();
        let quote: QuoteInfo = serde_json::from_str(quote_value)?;

        Ok(quote)
    }

    pub fn update_quote_state(&self, quote_id: Uuid, quote_state: QuoteState) -> Result<QuoteInfo> {
        let write_txn = self.db.begin_write()?;

        let current_quote;

        {
            let mut quote: QuoteInfo;
            let mut quote_table = write_txn.open_table(QUOTES_TABLE)?;
            {
                let quote_value = quote_table
                    .get(quote_id.into_bytes().as_slice())?
                    .ok_or(anyhow!("Unknown quote"))?;

                let quote_value = quote_value.value();

                quote = serde_json::from_str(quote_value)?;
            }

            current_quote = quote.clone();

            quote.state = quote_state;

            quote_table.insert(
                quote_id.into_bytes().as_slice(),
                serde_json::to_string(&quote)?.as_str(),
            )?;
        }

        write_txn.commit()?;

        Ok(current_quote)
    }
}
