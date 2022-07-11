use akula::{
    accessors::{chain, state},
    consensus::engine_factory,
    execution::{
        analysis_cache::AnalysisCache, evmglue, processor::ExecutionProcessor, tracer::NoopTracer,
    },
    kv::{mdbx::*, tables, MdbxWithDirHandle},
    models::*,
    rpc::helpers,
    stagedsync::stages::{self, FINISH},
    Buffer, IntraBlockState,
};
use anyhow::format_err;
use async_trait::async_trait;
use ethereum_jsonrpc::{
    types::{self, TransactionLog},
    EthApiServer, LogFilter, SyncStatus,
};
use std::sync::Arc;

#[derive(Debug)]
pub struct DbWrapper<DB>
where
    DB: EnvironmentKind,
{
    db: Arc<MdbxWithDirHandle<DB>>,
    call_gas_limit: u64,
}

impl<DB> DbWrapper<DB>
where
    DB: EnvironmentKind,
{
    pub async fn call(
        &self,
        call_data: types::MessageCall,
        block_number: types::BlockId,
    ) -> anyhow::Result<types::Bytes> {
        let txn = self.db.begin()?;

        let (block_number, block_hash) = helpers::resolve_block_id(&txn, block_number)?
            .ok_or_else(|| format_err!("failed to resolve block {block_number:?}"))?;

        let header = chain::header::read(&txn, block_hash, block_number)?
            .ok_or_else(|| format_err!("Header not found for #{block_number}/{block_hash}"))?;

        let mut buffer = Buffer::new(&txn, Some(block_number));
        let mut state = IntraBlockState::new(&mut buffer);

        let mut analysis_cache = AnalysisCache::default();
        let block_spec = chain::chain_config::read(&txn)?
            .ok_or_else(|| format_err!("no chainspec found"))?
            .collect_block_spec(block_number);

        let sender = call_data.from.unwrap_or_else(Address::zero);

        let gas_limit = call_data
            .gas
            .map(|v| v.as_u64())
            .unwrap_or(self.call_gas_limit);

        let message = Message::Legacy {
            chain_id: None,
            nonce: state.get_nonce(sender)?,
            gas_price: call_data.gas_price.unwrap_or_default(),
            gas_limit,
            action: TransactionAction::Call(call_data.to),
            value: call_data.value.unwrap_or_default(),
            input: call_data.data.unwrap_or_default().into(),
        };

        let mut tracer = NoopTracer;

        Ok(evmglue::execute(
            &mut state,
            &mut tracer,
            &mut analysis_cache,
            &PartialHeader::from(header),
            &block_spec,
            &message,
            sender,
            gas_limit,
        )?
        .output_data
        .into())
    }
}
