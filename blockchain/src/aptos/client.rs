use std::{error::Error, str::FromStr};

use crate::{ChainProvider};
use async_trait::async_trait;
use chrono::Utc;
use num_bigint::BigUint;
use primitives::{chain::Chain, TransactionType, TransactionState, TransactionDirection, asset_id::AssetId};
use reqwest_middleware::ClientWithMiddleware;

use super::model::{Ledger, Block};

pub struct AptosClient {
    url: String,
    client: ClientWithMiddleware,
}

impl AptosClient {
    pub fn new(client: ClientWithMiddleware, url: String) -> Self {
        Self {
            url,
            client,
        }
    }

    pub fn map_transaction(&self, transaction: super::model::Transaction, block_number: i64) -> Option<primitives::Transaction> {
        let events = transaction.clone().events.unwrap_or_default();

        if transaction.transaction_type == "user_transaction" && events.len() == 2 && events[1].event_type == "0x1::coin::DepositEvent" {
            let asset_id = AssetId::from_chain(self.get_chain());
            let state = if transaction.success { TransactionState::Confirmed } else { TransactionState::Failed} ;
            let events = transaction.events.unwrap();
            let to = &events[1].guid.account_address;
            let value = &events[1].data.clone().unwrap().amount.unwrap_or_default();
            let gas_used = BigUint::from_str(transaction.gas_used.unwrap_or_default().as_str()).unwrap_or_default();
            let gas_unit_price = BigUint::from_str(transaction.gas_unit_price.unwrap_or_default().as_str()).unwrap_or_default();
            let fee = gas_used * gas_unit_price;
            
            let transaction = primitives::Transaction{
                id: "".to_string(),
                hash: transaction.hash,
                asset_id: asset_id.clone(),
                from: transaction.sender.unwrap_or_default(),
                to: to.clone(),
                contract: None,
                transaction_type: TransactionType::Transfer,
                state,
                block_number: block_number as i32,
                sequence: transaction.sequence_number.unwrap_or_default().parse::<i32>().unwrap_or_default(),
                fee: fee.to_string(),
                fee_asset_id: asset_id,
                value: value.clone(),
                memo: None,
                direction: TransactionDirection::SelfTransfer,
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            };        
            return Some(transaction)    
        }
        return None
    }

    pub async fn get_ledger(&self) -> Result<Ledger, Box<dyn Error + Send + Sync>> {
        let url = format!("{}/v1/", self.url);
        let response = self.client
            .get(url)
            .send()
            .await?
            .json::<Ledger>()
            .await?;
        return Ok(response)
    }

    pub async fn get_block_transactions(&self, block_number: i64) -> Result<Block, Box<dyn Error + Send + Sync>> {
        let url = format!("{}/v1/blocks/by_height/{}?with_transactions=true", self.url, block_number);
        let response = self.client
            .get(url)
            .send()
            .await?
            .json::<Block>()
            .await?;

        return Ok(response)
    }
}

#[async_trait]
impl ChainProvider for AptosClient {

    fn get_chain(&self) -> Chain {
        Chain::Aptos
    }

    async fn get_latest_block(&self) -> Result<i64, Box<dyn Error + Send + Sync>> {
        let ledger = self.get_ledger().await?;
        Ok(ledger.block_height.parse::<i64>().unwrap_or_default())
    }

    async fn get_transactions(&self, block_number: i64) -> Result<Vec<primitives::Transaction>, Box<dyn Error + Send + Sync>> {
        let transactions = self.get_block_transactions(block_number).await?.transactions;
        let transactions = transactions.into_iter()
            .flat_map(|x| self.map_transaction(x, block_number))
            .collect::<Vec<primitives::Transaction>>(); 

        Ok(transactions)
    }
}