use akula::{
    kv::{mdbx::*, tables, MdbxWithDirHandle},
    stagedsync::stages::FINISH,
};
use anyhow::format_err;
use async_trait::async_trait;
use ethers::{
    providers::{FromErr, Middleware},
    types::*,
};
use std::sync::Arc;
use thiserror::Error;

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
            db_wrapper: DbWrapper {
                db,
                call_gas_limit: 100_000_000,
            },
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

    async fn call(
        &self,
        tx: &ethers::types::transaction::eip2718::TypedTransaction,
        block: Option<ethers::types::BlockId>,
    ) -> Result<Bytes, Self::Error> {
        let block_id = if let Some(block_id) = block {
            utils::ethers_block_id_to_akula(block_id)
        } else {
            ethereum_jsonrpc::types::BlockId::Number(ethereum_jsonrpc::types::BlockNumber::Latest)
        };
        let message_call = utils::ethers_typed_tx_to_message_call(tx)?;

        self.db_wrapper
            .call(message_call, block_id)
            .await
            .map_or_else(
                |e| Err(AkulaMiddlewareError::DbWrapperError(e)),
                |res| Ok(ethers::types::Bytes::from(Vec::from(res.as_ref()))),
            )
    }
}
