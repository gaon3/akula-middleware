use akula::kv::{mdbx::*, MdbxWithDirHandle};
use async_trait::async_trait;
use ethers::{
    providers::{FromErr, Middleware},
    types::{transaction::eip2718::TypedTransaction, *},
};
use std::sync::Arc;
use thiserror::Error;

pub use ethereum_jsonrpc::types as jsonrpc;

use crate::{db_wrapper::DbWrapper, utils};

#[derive(Error, Debug)]
pub enum AkulaMiddlewareError<M: Middleware> {
    /// An error has occured while querying Akula's database.
    #[error(transparent)]
    DbWrapperError(#[from] anyhow::Error),
    /// A type conversion error has occured.
    #[error("failed to convert Akula type to ethers-rs type: {0}")]
    ConversionError(String),
    /// An error has occured in one of the middlewares.
    #[error("{0}")]
    MiddlewareError(M::Error),
}

impl<M: Middleware> FromErr<M::Error> for AkulaMiddlewareError<M> {
    fn from(err: M::Error) -> AkulaMiddlewareError<M> {
        AkulaMiddlewareError::MiddlewareError(err)
    }
}

#[derive(Debug)]
pub struct AkulaMiddleware<M, DB>
where
    DB: EnvironmentKind,
{
    inner: M,
    db_wrapper: DbWrapper<DB>,
}

impl<M, DB> AkulaMiddleware<M, DB>
where
    M: Middleware,
    DB: EnvironmentKind,
{
    pub fn new(inner: M, db: Arc<MdbxWithDirHandle<DB>>) -> Self {
        Self {
            inner,
            db_wrapper: DbWrapper::new(db, 100_000_000),
        }
    }
}

#[async_trait]
impl<M, DB> Middleware for AkulaMiddleware<M, DB>
where
    M: Middleware,
    DB: EnvironmentKind,
{
    type Error = AkulaMiddlewareError<M>;
    type Provider = M::Provider;
    type Inner = M;

    fn inner(&self) -> &M {
        &self.inner
    }

    async fn get_block_number(&self) -> Result<U64, Self::Error> {
        self.db_wrapper
            .block_number()
            .await
            .map_err(AkulaMiddlewareError::DbWrapperError)
    }

    async fn call(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockId>,
    ) -> Result<Bytes, Self::Error> {
        let block_id = block.map_or(
            jsonrpc::BlockId::Number(jsonrpc::BlockNumber::Latest),
            |block_id| utils::ethers_block_id_to_akula(block_id),
        );
        let message_call = utils::ethers_typed_tx_to_message_call(tx)?;

        self.db_wrapper
            .call(message_call, block_id)
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |v| Ok(Bytes::from(v.0)),
            )
    }

    async fn estimate_gas(&self, tx: &TypedTransaction) -> Result<U256, Self::Error> {
        let message_call = utils::ethers_typed_tx_to_message_call(tx)?;

        self.db_wrapper
            .estimate_gas(message_call, jsonrpc::BlockNumber::Latest)
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |v| Ok(U256::from(v.as_u64())),
            )
    }

    async fn get_balance<T>(&self, from: T, block: Option<BlockId>) -> Result<U256, Self::Error>
    where
        T: Into<NameOrAddress> + Send + Sync,
    {
        let from = match from.into() {
            NameOrAddress::Name(ens_name) => self
                .inner
                .resolve_name(&ens_name)
                .await
                .map_err(AkulaMiddlewareError::MiddlewareError)?,
            NameOrAddress::Address(addr) => addr,
        };
        let block_id = block.map_or(
            jsonrpc::BlockId::Number(jsonrpc::BlockNumber::Latest),
            |block_id| utils::ethers_block_id_to_akula(block_id),
        );

        self.db_wrapper
            .get_balance(from, block_id)
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |v| Ok(utils::ethnum_u256_to_ethers(&v)),
            )
    }

    async fn get_block<T>(
        &self,
        block_hash_or_number: T,
    ) -> Result<Option<Block<TxHash>>, Self::Error>
    where
        T: Into<BlockId> + Send + Sync,
    {
        let block_id = utils::ethers_block_id_to_akula(block_hash_or_number.into());
        self.db_wrapper
            .get_block(block_id, false)
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |v| Ok(v.map(|block| utils::jsonrpc_block_with_hashes_to_ethers(block))),
            )
    }

    async fn get_block_with_txs<T>(
        &self,
        block_hash_or_number: T,
    ) -> Result<Option<Block<Transaction>>, Self::Error>
    where
        T: Into<BlockId> + Send + Sync,
    {
        let block_id = utils::ethers_block_id_to_akula(block_hash_or_number.into());

        self.db_wrapper.get_block(block_id, true).await.map_or_else(
            |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
            |v| Ok(v.map(|block| utils::jsonrpc_block_with_txs_to_ethers(block))),
        )
    }

    async fn get_transaction<T: Into<TxHash> + Send + Sync>(
        &self,
        transaction_hash: T,
    ) -> Result<Option<Transaction>, Self::Error> {
        self.db_wrapper
            .get_transaction_by_hash(transaction_hash.into())
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |v| Ok(v.map(|tx| utils::jsonrpc_tx_to_ethers(&tx))),
            )
    }

    async fn get_transaction_count<T>(
        &self,
        from: T,
        block_id: Option<BlockId>,
    ) -> Result<U256, Self::Error>
    where
        T: Into<NameOrAddress> + Send + Sync,
    {
        let from = match from.into() {
            NameOrAddress::Name(ens_name) => self
                .inner
                .resolve_name(&ens_name)
                .await
                .map_err(AkulaMiddlewareError::MiddlewareError)?,
            NameOrAddress::Address(addr) => addr,
        };
        let block_id = block_id.map_or(
            jsonrpc::BlockId::Number(jsonrpc::BlockNumber::Latest),
            |block_id| utils::ethers_block_id_to_akula(block_id),
        );

        self.db_wrapper
            .get_transaction_count(from, block_id)
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |v| Ok(U256::from(v.as_u64())),
            )
    }

    async fn get_storage_at<T>(
        &self,
        from: T,
        location: H256,
        block: Option<BlockId>,
    ) -> Result<H256, Self::Error>
    where
        T: Into<NameOrAddress> + Send + Sync,
    {
        let at = match from.into() {
            NameOrAddress::Name(ens_name) => self
                .inner
                .resolve_name(&ens_name)
                .await
                .map_err(AkulaMiddlewareError::MiddlewareError)?,
            NameOrAddress::Address(addr) => addr,
        };
        let block_id = block.map_or(
            jsonrpc::BlockId::Number(jsonrpc::BlockNumber::Latest),
            |block_id| utils::ethers_block_id_to_akula(block_id),
        );

        self.db_wrapper
            .get_storage_at(
                at,
                akula::models::U256::from_be_bytes(*location.as_fixed_bytes()),
                block_id,
            )
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |v| Ok(H256(v.to_be_bytes())),
            )
    }

    async fn get_code<T>(&self, at: T, block: Option<BlockId>) -> Result<Bytes, Self::Error>
    where
        T: Into<NameOrAddress> + Send + Sync,
    {
        let at = match at.into() {
            NameOrAddress::Name(ens_name) => self
                .inner
                .resolve_name(&ens_name)
                .await
                .map_err(AkulaMiddlewareError::MiddlewareError)?,
            NameOrAddress::Address(addr) => addr,
        };
        let block_id = block.map_or(
            jsonrpc::BlockId::Number(jsonrpc::BlockNumber::Latest),
            |block_id| utils::ethers_block_id_to_akula(block_id),
        );

        self.db_wrapper.get_code(at, block_id).await.map_or_else(
            |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
            |v| Ok(Bytes::from(v.0)),
        )
    }

    async fn get_transaction_receipt<T: Into<TxHash> + Sync + Send>(
        &self,
        transaction_hash: T,
    ) -> Result<Option<TransactionReceipt>, Self::Error> {
        self.db_wrapper
            .get_transaction_receipt(transaction_hash.into())
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |v| Ok(v.map(|receipt| utils::jsonrpc_receipt_to_ethers(&receipt))),
            )
    }

    async fn get_uncle_count<T: Into<BlockId> + Send + Sync>(
        &self,
        block_hash_or_number: T,
    ) -> Result<U256, Self::Error> {
        self.db_wrapper
            .get_uncle_count(utils::ethers_block_id_to_akula(block_hash_or_number.into()))
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |v| Ok(U256::from(v.as_u64())),
            )
    }

    async fn get_uncle<T: Into<BlockId> + Send + Sync>(
        &self,
        block_hash_or_number: T,
        idx: U64,
    ) -> Result<Option<Block<H256>>, Self::Error> {
        self.db_wrapper
            .get_uncle_by_block_number_and_index(
                utils::ethers_block_id_to_akula(block_hash_or_number.into()),
                idx,
            )
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |v| Ok(v.map(|uncle| utils::jsonrpc_block_with_hashes_to_ethers(uncle))),
            )
    }
}
