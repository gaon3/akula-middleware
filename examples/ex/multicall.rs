pub use multicall_mod::*;
#[allow(clippy::too_many_arguments)]
mod multicall_mod {
    #![allow(clippy::enum_variant_names)]
    #![allow(dead_code)]
    #![allow(clippy::type_complexity)]
    #![allow(unused_imports)]
    use ethers::{
        contract::{
            builders::{ContractCall, Event},
            Contract, Lazy,
        },
        core::{
            abi::{Abi, Detokenize, InvalidOutputType, Token, Tokenizable},
            types::*,
        },
        providers::Middleware,
    };
    #[doc = "Multicall was auto-generated with ethers-rs Abigen. More information at: https://github.com/gakonst/ethers-rs"]
    use std::sync::Arc;
    pub static MULTICALL_ABI: ethers::contract::Lazy<ethers::core::abi::Abi> =
        ethers::contract::Lazy::new(|| {
            serde_json :: from_str ("[\n\t{\n\t\t\"inputs\": [\n\t\t\t{\n\t\t\t\t\"components\": [\n\t\t\t\t\t{\n\t\t\t\t\t\t\"internalType\": \"address\",\n\t\t\t\t\t\t\"name\": \"target\",\n\t\t\t\t\t\t\"type\": \"address\"\n\t\t\t\t\t},\n\t\t\t\t\t{\n\t\t\t\t\t\t\"internalType\": \"bytes\",\n\t\t\t\t\t\t\"name\": \"callData\",\n\t\t\t\t\t\t\"type\": \"bytes\"\n\t\t\t\t\t}\n\t\t\t\t],\n\t\t\t\t\"internalType\": \"struct Multicall.Call[]\",\n\t\t\t\t\"name\": \"calls\",\n\t\t\t\t\"type\": \"tuple[]\"\n\t\t\t}\n\t\t],\n\t\t\"name\": \"aggregate\",\n\t\t\"outputs\": [\n\t\t\t{\n\t\t\t\t\"internalType\": \"uint256\",\n\t\t\t\t\"name\": \"blockNumber\",\n\t\t\t\t\"type\": \"uint256\"\n\t\t\t},\n\t\t\t{\n\t\t\t\t\"internalType\": \"bytes[]\",\n\t\t\t\t\"name\": \"returnData\",\n\t\t\t\t\"type\": \"bytes[]\"\n\t\t\t}\n\t\t],\n\t\t\"stateMutability\": \"nonpayable\",\n\t\t\"type\": \"function\"\n\t},\n\t{\n\t\t\"inputs\": [\n\t\t\t{\n\t\t\t\t\"internalType\": \"bool\",\n\t\t\t\t\"name\": \"requireSuccess\",\n\t\t\t\t\"type\": \"bool\"\n\t\t\t},\n\t\t\t{\n\t\t\t\t\"components\": [\n\t\t\t\t\t{\n\t\t\t\t\t\t\"internalType\": \"address\",\n\t\t\t\t\t\t\"name\": \"target\",\n\t\t\t\t\t\t\"type\": \"address\"\n\t\t\t\t\t},\n\t\t\t\t\t{\n\t\t\t\t\t\t\"internalType\": \"bytes\",\n\t\t\t\t\t\t\"name\": \"callData\",\n\t\t\t\t\t\t\"type\": \"bytes\"\n\t\t\t\t\t}\n\t\t\t\t],\n\t\t\t\t\"internalType\": \"struct Multicall.Call[]\",\n\t\t\t\t\"name\": \"calls\",\n\t\t\t\t\"type\": \"tuple[]\"\n\t\t\t}\n\t\t],\n\t\t\"name\": \"tryAggregate\",\n\t\t\"outputs\": [\n\t\t\t{\n\t\t\t\t\"components\": [\n\t\t\t\t\t{\n\t\t\t\t\t\t\"internalType\": \"bool\",\n\t\t\t\t\t\t\"name\": \"success\",\n\t\t\t\t\t\t\"type\": \"bool\"\n\t\t\t\t\t},\n\t\t\t\t\t{\n\t\t\t\t\t\t\"internalType\": \"bytes\",\n\t\t\t\t\t\t\"name\": \"returnData\",\n\t\t\t\t\t\t\"type\": \"bytes\"\n\t\t\t\t\t}\n\t\t\t\t],\n\t\t\t\t\"internalType\": \"struct Multicall.Result[]\",\n\t\t\t\t\"name\": \"returnData\",\n\t\t\t\t\"type\": \"tuple[]\"\n\t\t\t}\n\t\t],\n\t\t\"stateMutability\": \"nonpayable\",\n\t\t\"type\": \"function\"\n\t}\n]\n") . expect ("invalid abi")
        });
    pub struct Multicall<M>(ethers::contract::Contract<M>);
    impl<M> Clone for Multicall<M> {
        fn clone(&self) -> Self {
            Multicall(self.0.clone())
        }
    }
    impl<M> std::ops::Deref for Multicall<M> {
        type Target = ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M: ethers::providers::Middleware> std::fmt::Debug for Multicall<M> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_tuple(stringify!(Multicall))
                .field(&self.address())
                .finish()
        }
    }
    impl<'a, M: ethers::providers::Middleware> Multicall<M> {
        #[doc = r" Creates a new contract instance with the specified `ethers`"]
        #[doc = r" client at the given `Address`. The contract derefs to a `ethers::Contract`"]
        #[doc = r" object"]
        pub fn new<T: Into<ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            ethers::contract::Contract::new(address.into(), MULTICALL_ABI.clone(), client).into()
        }
        #[doc = "Calls the contract's `aggregate` (0x252dba42) function"]
        pub fn aggregate(
            &self,
            calls: ::std::vec::Vec<CallInfo>,
        ) -> ethers::contract::builders::ContractCall<
            M,
            (
                ethers::core::types::U256,
                ::std::vec::Vec<ethers::core::types::Bytes>,
            ),
        > {
            self.0
                .method_hash([37, 45, 186, 66], calls)
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `tryAggregate` (0xbce38bd7) function"]
        pub fn try_aggregate(
            &self,
            require_success: bool,
            calls: ::std::vec::Vec<CallInfo>,
        ) -> ethers::contract::builders::ContractCall<M, ::std::vec::Vec<CallResult>> {
            self.0
                .method_hash([188, 227, 139, 215], (require_success, calls))
                .expect("method not found (this should never happen)")
        }
    }
    impl<M: ethers::providers::Middleware> From<ethers::contract::Contract<M>> for Multicall<M> {
        fn from(contract: ethers::contract::Contract<M>) -> Self {
            Self(contract)
        }
    }
    #[doc = "Container type for all input parameters for the `aggregate`function with signature `aggregate((address,bytes)[])` and selector `[37, 45, 186, 66]`"]
    #[derive(
        Clone,
        Debug,
        Default,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
    )]
    #[ethcall(name = "aggregate", abi = "aggregate((address,bytes)[])")]
    pub struct AggregateCall {
        pub calls: ::std::vec::Vec<CallInfo>,
    }
    #[doc = "Container type for all input parameters for the `tryAggregate`function with signature `tryAggregate(bool,(address,bytes)[])` and selector `[188, 227, 139, 215]`"]
    #[derive(
        Clone,
        Debug,
        Default,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
    )]
    #[ethcall(name = "tryAggregate", abi = "tryAggregate(bool,(address,bytes)[])")]
    pub struct TryAggregateCall {
        pub require_success: bool,
        pub calls: ::std::vec::Vec<CallInfo>,
    }
    #[derive(Debug, Clone, PartialEq, Eq, ethers :: contract :: EthAbiType)]
    pub enum MulticallCalls {
        Aggregate(AggregateCall),
        TryAggregate(TryAggregateCall),
    }
    impl ethers::core::abi::AbiDecode for MulticallCalls {
        fn decode(data: impl AsRef<[u8]>) -> Result<Self, ethers::core::abi::AbiError> {
            if let Ok(decoded) =
                <AggregateCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(MulticallCalls::Aggregate(decoded));
            }
            if let Ok(decoded) =
                <TryAggregateCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(MulticallCalls::TryAggregate(decoded));
            }
            Err(ethers::core::abi::Error::InvalidData.into())
        }
    }
    impl ethers::core::abi::AbiEncode for MulticallCalls {
        fn encode(self) -> Vec<u8> {
            match self {
                MulticallCalls::Aggregate(element) => element.encode(),
                MulticallCalls::TryAggregate(element) => element.encode(),
            }
        }
    }
    impl ::std::fmt::Display for MulticallCalls {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                MulticallCalls::Aggregate(element) => element.fmt(f),
                MulticallCalls::TryAggregate(element) => element.fmt(f),
            }
        }
    }
    impl ::std::convert::From<AggregateCall> for MulticallCalls {
        fn from(var: AggregateCall) -> Self {
            MulticallCalls::Aggregate(var)
        }
    }
    impl ::std::convert::From<TryAggregateCall> for MulticallCalls {
        fn from(var: TryAggregateCall) -> Self {
            MulticallCalls::TryAggregate(var)
        }
    }
    #[doc = "`Call(address,bytes)`"]
    #[derive(
        Clone,
        Debug,
        Default,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
    )]
    pub struct CallInfo {
        pub target: ethers::core::types::Address,
        pub call_data: ethers::core::types::Bytes,
    }

    impl From<(ethers::core::types::Address, ethers::core::types::Bytes)> for CallInfo {
        fn from(tuple: (ethers::core::types::Address, ethers::core::types::Bytes)) -> Self {
            Self {
                target: tuple.0,
                call_data: tuple.1,
            }
        }
    }

    #[doc = "`Result(bool,bytes)`"]
    #[derive(
        Clone,
        Debug,
        Default,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
    )]
    pub struct CallResult {
        pub success: bool,
        pub return_data: ethers::core::types::Bytes,
    }
}
