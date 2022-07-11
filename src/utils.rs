use akula::{
    binutil::AkulaDataDir,
    kv::{mdbx::*, MdbxWithDirHandle},
};
use anyhow::format_err;
use ethereum_jsonrpc::types as akula_types;
use ethers::{providers::Middleware, types as ethers_types};

use crate::middleware::{AkulaMiddleware, AkulaMiddlewareError};

pub fn open_database(
    db_path: impl Into<AkulaDataDir>,
) -> anyhow::Result<MdbxWithDirHandle<NoWriteMap>> {
    let db: MdbxWithDirHandle<NoWriteMap> = MdbxEnvironment::<NoWriteMap>::open_ro(
        akula::kv::Environment::new(),
        &db_path.into(),
        akula::kv::tables::CHAINDATA_TABLES.clone(),
    )?
    .into();

    Ok(db)
}

pub fn ethers_block_id_to_akula(block_id: ethers_types::BlockId) -> akula_types::BlockId {
    match block_id {
        ethers_types::BlockId::Number(number) => match number {
            ethers_types::BlockNumber::Latest => {
                akula_types::BlockId::Number(akula_types::BlockNumber::Latest)
            }
            ethers_types::BlockNumber::Earliest => {
                akula_types::BlockId::Number(akula_types::BlockNumber::Earliest)
            }
            ethers_types::BlockNumber::Pending => {
                akula_types::BlockId::Number(akula_types::BlockNumber::Latest)
            }
            ethers_types::BlockNumber::Number(n) => {
                akula_types::BlockId::Number(akula_types::BlockNumber::Number(n))
            }
        },
        ethers_types::BlockId::Hash(hash) => akula_types::BlockId::Hash(hash),
    }
}

pub fn ethers_typed_tx_to_message_call<M: Middleware>(
    typed_transaction: &ethers_types::transaction::eip2718::TypedTransaction,
) -> Result<akula_types::MessageCall, AkulaMiddlewareError<M>> {
    let from = typed_transaction.from().map(|addr| *addr);
    let to = if let Some(to) = typed_transaction.to() {
        match to {
            ethers_types::NameOrAddress::Address(addr) => *addr,
            ethers_types::NameOrAddress::Name(_) => {
                return Err(AkulaMiddlewareError::ConversionError(String::from(
                    "can't convert None to Address",
                )));
            }
        }
    } else {
        return Err(AkulaMiddlewareError::ConversionError(String::from(
            "can't convert None to Address",
        )));
    };
    let gas = typed_transaction.gas().map(|gas| gas.as_u64().into());
    let gas_price = typed_transaction
        .gas_price()
        .as_ref()
        .map(|price| ethers_u256_to_ethnum(price));
    let value = typed_transaction.value().map(|v| ethers_u256_to_ethnum(v));
    let data = typed_transaction
        .data()
        .map(|data| akula_types::Bytes::from(data.to_vec()));
    Ok(akula_types::MessageCall {
        from,
        to,
        gas,
        gas_price,
        value,
        data,
    })
}

#[inline]
pub fn ethers_u256_to_ethnum(n: &ethers_types::U256) -> akula::models::U256 {
    let mut bytes: [u8; 32] = [0; 32];
    n.to_little_endian(&mut bytes);
    akula::models::U256::from_le_bytes(bytes)
}
