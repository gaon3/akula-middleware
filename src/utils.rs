use std::collections::HashSet;

use crate::middleware::AkulaMiddlewareError;
use akula::{
    binutil::AkulaDataDir,
    kv::{mdbx::*, MdbxWithDirHandle},
};
use ethereum_jsonrpc::types as jsonrpc;
use ethers::{providers::Middleware, types as ethers_types};

pub fn open_database(db_path: AkulaDataDir) -> anyhow::Result<MdbxWithDirHandle<NoWriteMap>> {
    let akula_chain_data_dir = db_path.chain_data_dir();
    let db: MdbxWithDirHandle<NoWriteMap> = MdbxEnvironment::<NoWriteMap>::open_ro(
        libmdbx::Environment::new(),
        &akula_chain_data_dir,
        akula::kv::tables::CHAINDATA_TABLES.clone(),
    )?
    .into();

    Ok(db)
}

pub fn ethers_block_id_to_akula(block_id: ethers_types::BlockId) -> jsonrpc::BlockId {
    match block_id {
        ethers_types::BlockId::Number(number) => match number {
            ethers_types::BlockNumber::Latest => {
                jsonrpc::BlockId::Number(jsonrpc::BlockNumber::Latest)
            }
            ethers_types::BlockNumber::Earliest => {
                jsonrpc::BlockId::Number(jsonrpc::BlockNumber::Earliest)
            }
            ethers_types::BlockNumber::Pending => {
                jsonrpc::BlockId::Number(jsonrpc::BlockNumber::Latest)
            }
            ethers_types::BlockNumber::Number(n) => {
                jsonrpc::BlockId::Number(jsonrpc::BlockNumber::Number(n))
            }
        },
        ethers_types::BlockId::Hash(hash) => jsonrpc::BlockId::Hash(hash),
    }
}

pub fn ethers_typed_tx_to_message_call<M: Middleware>(
    typed_transaction: &ethers_types::transaction::eip2718::TypedTransaction,
) -> Result<jsonrpc::MessageCall, AkulaMiddlewareError<M>> {
    let from = typed_transaction.from().map(|addr| *addr);
    let to = if let Some(to) = typed_transaction.to() {
        match to {
            ethers_types::NameOrAddress::Address(addr) => Some(*addr),
            ethers_types::NameOrAddress::Name(_) => {
                return Err(AkulaMiddlewareError::ConversionError(String::from(
                    "can't convert ENS name to Address",
                )));
            }
        }
    } else {
        None
    };
    let gas = typed_transaction.gas().map(|gas| gas.as_u64().into());
    let gas_price = typed_transaction
        .gas_price()
        .as_ref()
        .map(|price| ethers_u256_to_ethnum(price));
    let value = typed_transaction.value().map(|v| ethers_u256_to_ethnum(v));
    let data = typed_transaction
        .data()
        .map(|data| jsonrpc::Bytes::from(data.0.clone()));

    use ethers_types::transaction::eip2718;
    match typed_transaction {
        eip2718::TypedTransaction::Legacy(_) => Ok(jsonrpc::MessageCall::Legacy {
            from,
            to,
            gas,
            gas_price,
            value,
            data,
        }),
        eip2718::TypedTransaction::Eip2930(tx) => Ok(jsonrpc::MessageCall::EIP2930 {
            from,
            to,
            gas,
            gas_price,
            value,
            data,
            access_list: Some(
                tx.access_list
                    .0
                    .iter()
                    .map(|item| jsonrpc::AccessListEntry {
                        address: item.address,
                        storage_keys: HashSet::from_iter(item.storage_keys.iter().cloned()),
                    })
                    .collect(),
            ),
        }),
        eip2718::TypedTransaction::Eip1559(tx) => Ok(jsonrpc::MessageCall::EIP1559 {
            from,
            to,
            gas,
            max_fee_per_gas: tx.max_fee_per_gas.map(|v| ethers_u256_to_ethnum(&v)),
            max_priority_fee_per_gas: tx
                .max_priority_fee_per_gas
                .map(|v| ethers_u256_to_ethnum(&v)),
            value,
            data,
            access_list: Some(
                tx.access_list
                    .0
                    .iter()
                    .map(|item| jsonrpc::AccessListEntry {
                        address: item.address,
                        storage_keys: HashSet::from_iter(item.storage_keys.iter().cloned()),
                    })
                    .collect(),
            ),
        }),
    }
}

#[inline]
pub fn ethers_u256_to_ethnum(n: &ethers_types::U256) -> akula::models::U256 {
    let mut bytes: [u8; 32] = [0; 32];
    n.to_little_endian(&mut bytes);
    akula::models::U256::from_le_bytes(bytes)
}

#[inline]
pub fn ethnum_u256_to_ethers(n: &akula::models::U256) -> ethers_types::U256 {
    let bytes = n.to_le_bytes();
    ethers_types::U256::from_big_endian(&bytes)
}

pub fn jsonrpc_block_with_txs_to_ethers(
    block: jsonrpc::Block,
) -> ethers_types::Block<ethers_types::Transaction> {
    ethers_types::Block {
        hash: block.hash,
        parent_hash: block.parent_hash,
        author: Some(block.miner),
        state_root: block.state_root,
        transactions_root: block.transactions_root,
        receipts_root: block.receipts_root,
        number: block.number,
        gas_used: ethers_types::U256::from(block.gas_used.as_u64()),
        extra_data: ethers_types::Bytes::from(block.extra_data.0),
        logs_bloom: block.logs_bloom,
        timestamp: ethers_types::U256::from(block.timestamp.as_u64()),
        total_difficulty: block.total_difficulty.map(|v| ethnum_u256_to_ethers(&v)),
        seal_fields: vec![],
        transactions: block
            .transactions
            .iter()
            .filter_map(|tx| match tx {
                jsonrpc::Tx::Transaction(tx) => Some(jsonrpc_tx_to_ethers(tx.as_ref())),
                jsonrpc::Tx::Hash(_) => None,
            })
            .collect(),
        size: Some(ethers_types::U256::from(block.size.as_u64())),
        base_fee_per_gas: None,
        uncles_hash: block.sha3_uncles,
        gas_limit: ethers_types::U256::from(block.gas_limit.as_u64()),
        difficulty: ethnum_u256_to_ethers(&block.difficulty),
        uncles: block.uncles,
        mix_hash: block.mix_hash,
        nonce: block.nonce,
        other: ethers_types::OtherFields::default(),
    }
}

pub fn jsonrpc_block_with_hashes_to_ethers(
    block: jsonrpc::Block,
) -> ethers_types::Block<ethers_types::H256> {
    ethers_types::Block {
        hash: block.hash,
        parent_hash: block.parent_hash,
        author: Some(block.miner),
        state_root: block.state_root,
        transactions_root: block.transactions_root,
        receipts_root: block.receipts_root,
        number: block.number,
        gas_used: ethers_types::U256::from(block.gas_used.as_u64()),
        extra_data: ethers_types::Bytes::from(block.extra_data.0),
        logs_bloom: block.logs_bloom,
        timestamp: ethers_types::U256::from(block.timestamp.as_u64()),
        total_difficulty: block.total_difficulty.map(|v| ethnum_u256_to_ethers(&v)),
        seal_fields: vec![],
        transactions: block
            .transactions
            .iter()
            .filter_map(|tx| match tx {
                jsonrpc::Tx::Transaction(_) => None,
                jsonrpc::Tx::Hash(hash) => Some(*hash),
            })
            .collect(),
        size: Some(ethers_types::U256::from(block.size.as_u64())),
        base_fee_per_gas: None,
        uncles_hash: block.sha3_uncles,
        gas_limit: ethers_types::U256::from(block.gas_limit.as_u64()),
        difficulty: ethnum_u256_to_ethers(&block.difficulty),
        uncles: block.uncles,
        mix_hash: block.mix_hash,
        nonce: block.nonce,
        other: ethers_types::OtherFields::default(),
    }
}

pub fn jsonrpc_tx_to_ethers(tx: &jsonrpc::Transaction) -> ethers_types::Transaction {
    ethers_types::Transaction {
        hash: tx.hash,
        nonce: ethers_types::U256::from(tx.nonce.as_u64()),
        block_hash: tx.block_hash,
        block_number: tx.block_number,
        transaction_index: tx.transaction_index,
        from: tx.from,
        to: tx.to,
        value: ethnum_u256_to_ethers(&tx.value),
        gas: ethers_types::U256::from(tx.gas.as_u64()),
        gas_price: Some(ethnum_u256_to_ethers(&tx.gas_price)),
        input: ethers_types::Bytes::from(tx.input.0.clone()),
        v: tx.v,
        r: ethers_types::U256::from(tx.r.as_fixed_bytes()),
        s: ethers_types::U256::from(tx.s.as_fixed_bytes()),
        transaction_type: None,
        access_list: None,
        max_priority_fee_per_gas: None,
        max_fee_per_gas: None,
        chain_id: None,
        other: ethers_types::OtherFields::default(),
    }
}

pub fn jsonrpc_receipt_to_ethers(
    tx: &jsonrpc::TransactionReceipt,
) -> ethers_types::TransactionReceipt {
    ethers_types::TransactionReceipt {
        transaction_hash: tx.transaction_hash,
        block_hash: Some(tx.block_hash),
        block_number: Some(tx.block_number),
        transaction_index: tx.transaction_index,
        from: tx.from,
        to: tx.to,
        cumulative_gas_used: ethers_types::U256::from(tx.cumulative_gas_used.as_u64()),
        gas_used: Some(ethers_types::U256::from(tx.gas_used.as_u64())),
        contract_address: tx.contract_address,
        logs: tx
            .logs
            .iter()
            .map(|log| jsonrpc_log_to_ethers(log))
            .collect(),
        logs_bloom: tx.logs_bloom,
        status: Some(tx.status),
        effective_gas_price: None,
        transaction_type: None,
        root: None,
    }
}

pub fn jsonrpc_log_to_ethers(log: &jsonrpc::TransactionLog) -> ethers_types::Log {
    ethers_types::Log {
        address: log.address,
        topics: log.topics.clone(),
        data: ethers_types::Bytes::from(log.data.0.clone()),
        block_hash: log.block_hash,
        block_number: log.block_number,
        transaction_hash: log.transaction_hash,
        transaction_index: log.transaction_index,
        log_index: log.log_index.map(|v| ethers_types::U256::from(v.as_u64())),
        transaction_log_index: None,
        log_type: None,
        removed: None,
    }
}
