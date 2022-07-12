use akula_middleware::AkulaMiddleware;
use ethers::prelude::*;
use ethers::{abi::AbiEncode, contract::abigen};
use std::sync::Arc;

mod erc20_token;
use erc20_token::ERC20Token;
mod multicall;
use multicall::{CallInfo, Multicall};

const SIG_ALL_PAIRS: [u8; 4] = [0x1e, 0x3d, 0xd1, 0x8b];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = "/akula-db";
    let db = akula_middleware::open_database(path.parse()?)?;

    let provider = Provider::<Http>::try_from(
        "https://eth-mainnet.g.alchemy.com/v2/mG-WsFAIf4ZL3oMa17m-q4RED_KsmObX",
    )?;
    let provider = Arc::new(provider);

    let dai_addr: Address = "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse()?;
    let dai = ERC20Token::new(dai_addr, provider.clone());

    println!("{:?}", dai.decimals().call().await?);

    let multicall1 = Multicall::new(
        "0x5BA1e12693Dc8F9c48aAD8770482f4739bEeD696".parse::<Address>()?,
        Arc::clone(&provider),
    );

    let calldata = vec![CallInfo {
        target: dai_addr,
        call_data: dai.encode("decimals", ())?,
    }];
    let (_, returndata) = multicall1.aggregate(calldata.clone()).call().await?;
    println!("{:?}", U256::from_big_endian(returndata[0].as_ref()));

    let provider = AkulaMiddleware::new(provider, Arc::new(db));
    let provider = Arc::new(provider);

    println!("{:?}", dai.decimals().call().await?);

    let multicall2 = Multicall::new(
        "0x5BA1e12693Dc8F9c48aAD8770482f4739bEeD696".parse::<Address>()?,
        Arc::clone(&provider),
    );

    let (_, returndata) = multicall2.aggregate(calldata).call().await?;
    println!("{:?}", U256::from_big_endian(returndata[0].as_ref()));
    // let call_data = (0..10)
    //     .into_iter()
    //     .map(|i| {
    //         let mut data = SIG_ALL_PAIRS.to_vec();
    //         data.extend(i.encode());
    //         CallInfo {
    //             target: univ2_factory_addr,
    //             call_data: Bytes::from(data),
    //         }
    //     })
    //     .inspect(|d| println!("{:?}", d.call_data.to_vec()))
    //     .collect::<Vec<_>>();

    // let (_, returndata) = multicall.aggregate(call_data).call().await?;

    // for data in returndata {
    //     println!("{}", Address::from_slice(data.as_ref()));
    // }

    //let now = std::time::Instant::now();

    Ok(())
}
