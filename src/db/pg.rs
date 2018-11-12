use super::*;

use dotenv::dotenv;
use postgres::{Connection, TlsMode};
use std::{env, str::FromStr};

pub fn establish_connection() -> Result<Connection> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;
    Ok(Connection::connect(database_url, TlsMode::None)?)
}

pub struct Postresql {
    // TODO: pool
    connection: Connection,
}

impl Postresql {
    pub fn new() -> Result<Self> {
        let connection = establish_connection()?;
        Ok(Postresql { connection })
    }
}

impl DataStore for Postresql {
    fn get_max_height(&self) -> Result<Option<BlockHeight>> {
        Ok(self
            .connection
            .query("SELECT MAX(height) FROM blocks", &[])?
            .iter()
            .next()
            .and_then(|row| row.get::<_, Option<i64>>(0))
            .map(|u| u as u64))
    }

    fn get_hash_by_height(&self, height: BlockHeight) -> Result<Option<BlockHash>> {
        Ok(self
            .connection
            .query(
                "SELECT hash FROM blocks WHERE height = $1",
                &[&(height as i64)],
            )?
            .iter()
            .next()
            .map(|row| BlockHash::from(row.get::<_, Vec<u8>>(0).as_slice())))
    }

    fn reorg_at_height(&mut self, height: BlockHeight) -> Result<()> {
        self.connection
            .execute("REMOVE FROM blocks WHERE height >= $1", &[&(height as i64)])?;
        self.connection
            .execute("REMOVE FROM txs WHERE height >= $1", &[&(height as i64)])?;
        self.connection
            .execute("REMOVE FROM inputs WHERE height >= $1", &[&(height as i64)])?;
        self.connection.execute(
            "REMOVE FROM inputs WHERE outputs >= $1",
            &[&(height as i64)],
        )?;
        Ok(())
    }

    fn insert(&mut self, info: &BlockInfo) -> Result<()> {
        let (block, txs, outputs, inputs) = super::parse_node_block(info)?;

        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO blocks (height, hash, prev_hash) VALUES ($1, $2, $3)",
            &[
                &(block.height as i64),
                &block.hash.as_bytes().as_ref(),
                &block.prev_hash.as_bytes().as_ref(),
            ],
        )?;

        for tx in &txs {
            transaction.execute(
                "INSERT INTO txs (height, hash, coinbase) VALUES ($1, $2, $3)",
                &[
                    &(tx.height as i64),
                    &tx.hash.as_bytes().as_ref(),
                    &tx.coinbase,
                ],
            )?;
        }

        for input in &inputs {
            transaction.execute(
                "INSERT INTO inputs (height, utxo_tx_hash, utxo_tx_idx) VALUES ($1, $2, $3)",
                &[
                    &(input.height as i64),
                    &input.utxo_tx_hash.as_bytes().as_ref(),
                    &(input.utxo_tx_idx as i32),
                ],
            )?;
        }

        for output in &outputs {
            transaction.execute(
                "INSERT INTO outputs (height, tx_hash, tx_idx, value, address, coinbase) VALUES ($1, $2, $3, $4, $5, $6)",
                &[
                    &(output.height as i64),
                    &output.tx_hash.as_bytes().as_ref(),
                    &(output.tx_idx as i32),
                    &(output.value as i64),
                    &output.address,
                    &output.coinbase,
                ],
            )?;
        }

        transaction.commit()?;

        Ok(())
    }
}