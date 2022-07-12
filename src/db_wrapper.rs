use akula::{
    accessors::{chain, state},
    consensus::engine_factory,
    execution::{
        analysis_cache::AnalysisCache, evmglue, processor::ExecutionProcessor, tracer::NoopTracer,
    },
    kv::{mdbx::*, tables, MdbxWithDirHandle},
    models::*,
    rpc::helpers,
    stagedsync::stages::FINISH,
    Buffer, IntraBlockState,
};
use anyhow::format_err;
use ethereum_jsonrpc::types;
use std::sync::Arc;

#[derive(Debug)]
pub struct DbWrapper<DB>
where
    DB: EnvironmentKind,
{
    db: Arc<MdbxWithDirHandle<DB>>,
    call_gas_limit: u64,
}
impl<DB: EnvironmentKind> DbWrapper<DB> {
    pub fn new(db: Arc<MdbxWithDirHandle<DB>>, call_gas_limit: u64) -> Self {
        Self { db, call_gas_limit }
    }
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
        let chain_id = txn
            .get(tables::Config, ())?
            .ok_or_else(|| format_err!("chain spec not found"))?
            .params
            .chain_id;

        let header = chain::header::read(&txn, block_hash, block_number)?
            .ok_or_else(|| format_err!("Header not found for #{block_number}/{block_hash}"))?
            .into();

        let mut buffer = Buffer::new(&txn, Some(block_number));

        let (sender, message) = helpers::convert_message_call(
            &buffer,
            chain_id,
            call_data,
            &header,
            U256::ZERO,
            Some(self.call_gas_limit),
        )?;

        let mut state = IntraBlockState::new(&mut buffer);

        let mut analysis_cache = AnalysisCache::default();
        let block_spec = chain::chain_config::read(&txn)?
            .ok_or_else(|| format_err!("no chainspec found"))?
            .collect_block_spec(block_number);

        let mut tracer = NoopTracer;

        Ok(evmglue::execute(
            &mut state,
            &mut tracer,
            &mut analysis_cache,
            &header,
            &block_spec,
            &message,
            sender,
            message.gas_limit(),
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

        let chain_id = txn
            .get(tables::Config, ())?
            .ok_or_else(|| format_err!("chain spec not found"))?
            .params
            .chain_id;
        let header = chain::header::read(&txn, hash, block_number)?
            .ok_or_else(|| format_err!("no header found for block #{block_number}/{hash}"))?
            .into();
        let mut buffer = Buffer::new(&txn, Some(block_number));

        let (sender, message) =
            helpers::convert_message_call(&buffer, chain_id, call_data, &header, U256::ZERO, None)?;

        let mut state = IntraBlockState::new(&mut buffer);

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
                    &header,
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

    pub async fn get_transaction_count(
        &self,
        address: Address,
        block_id: types::BlockId,
    ) -> anyhow::Result<U64> {
        let txn = self.db.begin()?;
        let (block_number, _) = helpers::resolve_block_id(&txn, block_id)?
            .ok_or_else(|| format_err!("failed to resolve block {block_id:?}"))?;

        Ok(state::account::read(&txn, address, Some(block_number))?
            .map(|account| account.nonce)
            .unwrap_or(0)
            .into())
    }

    pub async fn get_transaction_receipt(
        &self,
        hash: H256,
    ) -> anyhow::Result<Option<types::TransactionReceipt>> {
        let txn = self.db.begin()?;

        if let Some(block_number) = chain::tl::read(&txn, hash)? {
            let block_hash = chain::canonical_hash::read(&txn, block_number)?
                .ok_or_else(|| format_err!("no canonical header for block #{block_number:?}"))?;
            let header = PartialHeader::from(
                chain::header::read(&txn, block_hash, block_number)?.ok_or_else(|| {
                    format_err!("header not found for block #{block_number}/{block_hash}")
                })?,
            );
            let block_body = chain::block_body::read_with_senders(&txn, block_hash, block_number)?
                .ok_or_else(|| {
                    format_err!("body not found for block #{block_number}/{block_hash}")
                })?;
            let chain_spec = chain::chain_config::read(&txn)?
                .ok_or_else(|| format_err!("chain specification not found"))?;

            // Prepare the execution context.
            let mut buffer = Buffer::new(&txn, Some(BlockNumber(block_number.0 - 1)));

            let block_execution_spec = chain_spec.collect_block_spec(block_number);
            let mut engine = engine_factory(None, chain_spec)?;
            let mut analysis_cache = AnalysisCache::default();
            let mut tracer = NoopTracer;

            let mut processor = ExecutionProcessor::new(
                &mut buffer,
                &mut tracer,
                &mut analysis_cache,
                &mut *engine,
                &header,
                &block_body,
                &block_execution_spec,
            );

            let transaction_index = chain::block_body::read_without_senders(&txn, block_hash, block_number)?.ok_or_else(|| format_err!("where's block body"))?.transactions
                    .into_iter()
                    .enumerate()
                    .find(|(_, tx)| tx.hash() == hash)
                    .ok_or_else(|| format_err!("transaction {hash} not found in block #{block_number}/{block_hash} despite lookup index"))?.0;

            let receipts =
                processor.execute_block_no_post_validation_while(|i, _| i <= transaction_index)?;

            let transaction = &block_body.transactions[transaction_index];
            let receipt = receipts.get(transaction_index).unwrap();
            let gas_used = U64::from(
                receipt.cumulative_gas_used
                    - transaction_index
                        .checked_sub(1)
                        .and_then(|last_index| receipts.get(last_index))
                        .map(|receipt| receipt.cumulative_gas_used)
                        .unwrap_or(0),
            );
            let logs = receipt
                .logs
                .iter()
                .enumerate()
                .map(|(i, log)| types::TransactionLog {
                    log_index: Some(U64::from(i)),
                    transaction_index: Some(U64::from(transaction_index)),
                    transaction_hash: Some(transaction.hash()),
                    block_hash: Some(block_hash),
                    block_number: Some(U64::from(block_number.0)),
                    address: log.address,
                    data: log.data.clone().into(),
                    topics: log.topics.clone(),
                })
                .collect::<Vec<_>>();

            return Ok(Some(types::TransactionReceipt {
                transaction_hash: hash,
                transaction_index: U64::from(transaction_index),
                block_hash,
                block_number: U64::from(block_number.0),
                from: transaction.sender,
                to: transaction.message.action().into_address(),
                cumulative_gas_used: receipt.cumulative_gas_used.into(),
                gas_used,
                contract_address: if let TransactionAction::Create = transaction.message.action() {
                    Some(akula::execution::address::create_address(
                        transaction.sender,
                        transaction.message.nonce(),
                    ))
                } else {
                    None
                },
                logs,
                logs_bloom: receipt.bloom,
                status: if receipt.success {
                    U64::from(1_u16)
                } else {
                    U64::zero()
                },
            }));
        }

        Ok(None)
    }

    pub async fn get_uncle_by_block_number_and_index(
        &self,
        block_id: types::BlockId,
        index: U64,
    ) -> anyhow::Result<Option<types::Block>> {
        let txn = self.db.begin()?;
        Ok(helpers::construct_block(
            &txn,
            block_id,
            false,
            Some(index),
        )?)
    }

    pub async fn get_uncle_count(&self, block_id: types::BlockId) -> anyhow::Result<U64> {
        let txn = self.db.begin()?;
        let (block_number, block_hash) = helpers::resolve_block_id(&txn, block_id)?
            .ok_or_else(|| format_err!("failed to resolve block {block_id:?}"))?;

        Ok(U64::from(
            chain::storage_body::read(&txn, block_hash, block_number)?
                .map(|body| body.uncles.len())
                .unwrap_or(0),
        ))
    }
}
