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
    pub async fn block_number(&self) -> anyhow::Result<U64> {
        Ok(U64::from(
            self.db
                .begin()?
                .get(tables::SyncStage, FINISH)?
                .unwrap_or(BlockNumber(0))
                .0,
        ))
    }

    pub async fn call(
        &self,
        call_data: types::MessageCall,
        block_id: types::BlockId,
    ) -> anyhow::Result<types::Bytes> {
        let txn = self.db.begin()?;

        let (block_number, block_hash) = helpers::resolve_block_id(&txn, block_id)?
            .ok_or_else(|| format_err!("failed to resolve block {block_id:?}"))?;

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

    pub async fn estimate_gas(
        &self,
        call_data: types::MessageCall,
        block_number: types::BlockNumber,
    ) -> anyhow::Result<U64> {
        let txn = self.db.begin()?;
        let (block_number, hash) = helpers::resolve_block_id(&txn, block_number)?
            .ok_or_else(|| format_err!("failed to resolve block {block_number:?}"))?;
        let header = chain::header::read(&txn, hash, block_number)?
            .ok_or_else(|| format_err!("no header found for block #{block_number}/{hash}"))?;
        let mut buffer = Buffer::new(&txn, Some(block_number));
        let mut state = IntraBlockState::new(&mut buffer);
        let sender = call_data.from.unwrap_or_else(Address::zero);
        let message = Message::Legacy {
            chain_id: None,
            nonce: state.get_nonce(sender)?,
            gas_price: call_data
                .gas_price
                .map(|v| v.as_u64().as_u256())
                .unwrap_or(U256::ZERO),
            gas_limit: call_data
                .gas
                .map(|gas| gas.as_u64())
                .unwrap_or(header.gas_limit),
            action: TransactionAction::Call(call_data.to),
            value: call_data.value.unwrap_or(U256::ZERO),
            input: call_data.data.unwrap_or_default().into(),
        };
        let mut cache = AnalysisCache::default();
        let block_spec = chain::chain_config::read(&txn)?
            .ok_or_else(|| format_err!("no chainspec found"))?
            .collect_block_spec(block_number);
        let mut tracer = NoopTracer;
        let gas_limit = header.gas_limit;

        Ok(U64::from(
            gas_limit as i64
                - evmglue::execute(
                    &mut state,
                    &mut tracer,
                    &mut cache,
                    &PartialHeader::from(header),
                    &block_spec,
                    &message,
                    sender,
                    gas_limit,
                )?
                .gas_left,
        ))
    }

    pub async fn get_balance(
        &self,
        address: Address,
        block_id: types::BlockId,
    ) -> anyhow::Result<U256> {
        let txn = self.db.begin()?;

        let (block_number, _) = helpers::resolve_block_id(&txn, block_id)?
            .ok_or_else(|| format_err!("failed to resolve block {block_id:?}"))?;
        Ok(state::account::read(&txn, address, Some(block_number))?
            .map(|acc| acc.balance)
            .unwrap_or(U256::ZERO))
    }

    pub async fn get_block(
        &self,
        block_id: types::BlockId,
        include_txs: bool,
    ) -> anyhow::Result<Option<types::Block>> {
        Ok(helpers::construct_block(
            &self.db.begin()?,
            block_id,
            include_txs,
            None,
        )?)
    }

    pub async fn get_transaction_by_hash(
        &self,
        hash: H256,
    ) -> anyhow::Result<Option<types::Transaction>> {
        let txn = self.db.begin()?;
        if let Some(block_number) = chain::tl::read(&txn, hash)? {
            let block_hash = chain::canonical_hash::read(&txn, block_number)?
                .ok_or_else(|| format_err!("canonical hash for block #{block_number} not found"))?;
            let (index, transaction) = chain::block_body::read_without_senders(
                                &txn,
                                block_hash,
                                block_number,
                            )?.ok_or_else(|| format_err!("body not found for block #{block_number}/{block_hash}"))?
                        .transactions
                        .into_iter()
                        .enumerate()
                        .find(|(_, tx)| tx.hash() == hash)
                        .ok_or_else(|| {
                                    format_err!(
                                            "tx with hash {hash} is not found in block #{block_number}/{block_hash} - tx lookup index invalid?"
                                        )
                                })?;
            let senders = chain::tx_sender::read(&txn, block_hash, block_number)?;
            let sender = *senders
                .get(index)
                .ok_or_else(|| format_err!("senders to short: {index} vs len {}", senders.len()))?;
            return Ok(Some(types::Transaction {
                hash,
                nonce: transaction.nonce().into(),
                block_hash: Some(block_hash),
                block_number: Some(block_number.0.into()),
                from: sender,
                gas: transaction.gas_limit().into(),
                gas_price: match transaction.message {
                    Message::Legacy { gas_price, .. } => gas_price,
                    Message::EIP2930 { gas_price, .. } => gas_price,
                    Message::EIP1559 {
                        max_fee_per_gas, ..
                    } => max_fee_per_gas,
                },
                input: transaction.input().clone().into(),
                to: match transaction.action() {
                    TransactionAction::Call(to) => Some(to),
                    TransactionAction::Create => None,
                },
                transaction_index: Some(U64::from(index)),
                value: transaction.value(),
                v: transaction.v().into(),
                r: transaction.r(),
                s: transaction.s(),
            }));
        }

        Ok(None)
    }

    pub async fn get_code(
        &self,
        address: Address,
        block_id: types::BlockId,
    ) -> anyhow::Result<types::Bytes> {
        let txn = self.db.begin()?;
        let (block_number, _) = helpers::resolve_block_id(&txn, block_id)?
            .ok_or_else(|| format_err!("failed to resolve block {block_id:?}"))?;
        Ok(
            if let Some(account) = state::account::read(&txn, address, Some(block_number))? {
                txn.get(tables::Code, account.code_hash)?
                    .ok_or_else(|| {
                        format_err!("failed to find code for code hash {}", account.code_hash)
                    })?
                    .into()
            } else {
                Default::default()
            },
        )
    }

    pub async fn get_storage_at(
        &self,
        address: Address,
        key: U256,
        block_id: types::BlockId,
    ) -> anyhow::Result<U256> {
        let txn = self.db.begin()?;
        let (block_number, _) = helpers::resolve_block_id(&txn, block_id)?
            .ok_or_else(|| format_err!("failed to resolve block {block_id:?}"))?;
        Ok(state::storage::read(
            &txn,
            address,
            key,
            Some(block_number),
        )?)
    }
}
