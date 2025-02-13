use std::io::Read;

use alloy::{
    consensus::{
        Block as PrimitiveBlock, BlockBody, Header as PrimitiveHeader,
        Transaction as PrimitiveTransaction, TxEnvelope as EthTxEnvelope,
    },
    eips::{eip2930::AccessList, eip7702::SignedAuthorization, Encodable2718, Typed2718},
    primitives::{Bytes, ChainId, TxKind, B256, U256},
    rpc::types::{
        engine::ExecutionPayload, Block as RpcBlock, BlockTransactions,
        Transaction as EthRpcTransaction,
    },
};
use clap::Parser;
use op_alloy::rpc_types::Transaction as OpRpcTransaction;
use serde::{Deserialize, Serialize};

/// Parses the given json file, creating an execution payload from it.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the json file to parse. If this is not specified, then stdin will be used.
    #[arg(short, long)]
    path: Option<String>,

    /// The engine rpc url to use
    #[arg(short, long)]
    rpc_url: Option<String>,

    /// The jwt secret to use
    #[arg(short, long)]
    jwt_secret: Option<String>,

    /// Output the raw payload, instead of including the command text. When used with stdin this
    /// can be very powerful, for example:
    ///
    /// ```sh
    /// cast block latest -r http://45.250.253.66:8544 --full -j | execution-payload-builder --raw | cast rpc --jwt-secret <JWT_SECRET> engine_newPayloadV3 --raw
    /// ```
    #[arg(long)]
    raw: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum RpcTransaction {
    Ethereum(EthRpcTransaction),
    Optimism(OpRpcTransaction),
}

impl Typed2718 for RpcTransaction {
    fn ty(&self) -> u8 {
        match self {
            RpcTransaction::Ethereum(tx) => tx.ty(),
            RpcTransaction::Optimism(tx) => tx.ty(),
        }
    }
}

impl PrimitiveTransaction for RpcTransaction {
    fn chain_id(&self) -> Option<ChainId> {
        match self {
            RpcTransaction::Ethereum(tx) => tx.chain_id(),
            RpcTransaction::Optimism(tx) => tx.chain_id(),
        }
    }

    fn nonce(&self) -> u64 {
        match self {
            RpcTransaction::Ethereum(tx) => tx.nonce(),
            RpcTransaction::Optimism(tx) => tx.nonce(),
        }
    }

    fn gas_limit(&self) -> u64 {
        match self {
            RpcTransaction::Ethereum(tx) => tx.gas_limit(),
            RpcTransaction::Optimism(tx) => tx.gas_limit(),
        }
    }

    fn gas_price(&self) -> Option<u128> {
        match self {
            RpcTransaction::Ethereum(tx) => tx.gas_price(),
            RpcTransaction::Optimism(tx) => tx.gas_price(),
        }
    }

    fn max_fee_per_gas(&self) -> u128 {
        match self {
            RpcTransaction::Ethereum(tx) => tx.max_fee_per_gas(),
            RpcTransaction::Optimism(tx) => tx.max_fee_per_gas(),
        }
    }

    fn max_priority_fee_per_gas(&self) -> Option<u128> {
        match self {
            RpcTransaction::Ethereum(tx) => tx.max_priority_fee_per_gas(),
            RpcTransaction::Optimism(tx) => tx.max_priority_fee_per_gas(),
        }
    }

    fn max_fee_per_blob_gas(&self) -> Option<u128> {
        match self {
            RpcTransaction::Ethereum(tx) => tx.max_fee_per_blob_gas(),
            RpcTransaction::Optimism(tx) => tx.max_fee_per_blob_gas(),
        }
    }

    fn priority_fee_or_price(&self) -> u128 {
        match self {
            RpcTransaction::Ethereum(tx) => tx.priority_fee_or_price(),
            RpcTransaction::Optimism(tx) => tx.priority_fee_or_price(),
        }
    }

    fn effective_gas_price(&self, base_fee: Option<u64>) -> u128 {
        match self {
            RpcTransaction::Ethereum(tx) => tx.effective_gas_price(base_fee),
            RpcTransaction::Optimism(tx) => tx.effective_gas_price(base_fee),
        }
    }

    fn is_dynamic_fee(&self) -> bool {
        match self {
            RpcTransaction::Ethereum(tx) => tx.is_dynamic_fee(),
            RpcTransaction::Optimism(tx) => tx.is_dynamic_fee(),
        }
    }

    fn kind(&self) -> TxKind {
        match self {
            RpcTransaction::Ethereum(tx) => tx.kind(),
            RpcTransaction::Optimism(tx) => tx.kind(),
        }
    }

    fn is_create(&self) -> bool {
        match self {
            RpcTransaction::Ethereum(tx) => tx.is_create(),
            RpcTransaction::Optimism(tx) => tx.is_create(),
        }
    }

    fn value(&self) -> U256 {
        match self {
            RpcTransaction::Ethereum(tx) => tx.value(),
            RpcTransaction::Optimism(tx) => tx.value(),
        }
    }

    fn input(&self) -> &Bytes {
        match self {
            RpcTransaction::Ethereum(tx) => tx.input(),
            RpcTransaction::Optimism(tx) => tx.input(),
        }
    }

    fn access_list(&self) -> Option<&AccessList> {
        match self {
            RpcTransaction::Ethereum(tx) => tx.access_list(),
            RpcTransaction::Optimism(tx) => tx.access_list(),
        }
    }

    fn blob_versioned_hashes(&self) -> Option<&[B256]> {
        match self {
            RpcTransaction::Ethereum(tx) => tx.blob_versioned_hashes(),
            RpcTransaction::Optimism(tx) => tx.blob_versioned_hashes(),
        }
    }

    fn authorization_list(&self) -> Option<&[SignedAuthorization]> {
        match self {
            RpcTransaction::Ethereum(tx) => tx.authorization_list(),
            RpcTransaction::Optimism(tx) => tx.authorization_list(),
        }
    }
}

impl Encodable2718 for RpcTransaction {
    fn encode_2718_len(&self) -> usize {
        match self {
            RpcTransaction::Ethereum(tx) => tx.inner.encode_2718_len(),
            RpcTransaction::Optimism(tx) => tx.inner.inner.encode_2718_len(),
        }
    }

    fn encode_2718(&self, out: &mut dyn alloy::primitives::bytes::BufMut) {
        match self {
            RpcTransaction::Ethereum(tx) => tx.inner.encode_2718(out),
            RpcTransaction::Optimism(tx) => tx.inner.inner.encode_2718(out),
        }
    }
}

fn main() {
    let args = Args::parse();

    // read the file specified in `--path` otherwise read from stdin
    let block_json = if let Some(path) = &args.path {
        std::fs::read_to_string(path).unwrap()
    } else {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer).unwrap();
        buffer
    };

    // parse the file
    let block: RpcBlock<RpcTransaction> = serde_json::from_str(&block_json).unwrap();

    // extract the parent beacon block root
    let parent_beacon_block_root = block.header.parent_beacon_block_root;

    // convert transactions into primitive txs
    // TODO: upstream into rpc compat
    let transactions = match block.transactions {
        // this would be an error in upstream
        BlockTransactions::Hashes(_hashes) => {
            panic!("send the eth_getBlockByHash request with full: `true`")
        }
        BlockTransactions::Full(txs) => txs,
        // this would be an error in upstream
        BlockTransactions::Uncle => panic!("this should not be run on uncle blocks"),
    };

    // extract blob versioned hashes from txs
    let blob_versioned_hashes = transactions
        .iter()
        .filter_map(|tx| {
            if let RpcTransaction::Ethereum(tx) = tx {
                if let EthTxEnvelope::Eip4844(tx) = &tx.inner {
                    return tx.tx().blob_versioned_hashes().map(|v| v.to_vec());
                }
            }
            None
        })
        .collect::<Vec<_>>();

    // convert to execution payload
    let execution_payload = ExecutionPayload::from_block_slow(&PrimitiveBlock::new(
        PrimitiveHeader::from(block.header),
        BlockBody {
            transactions,
            ommers: vec![],
            withdrawals: block.withdrawals,
        },
    ))
    .0;

    // create a JSON string for the request
    let json_request = serde_json::to_string(&(
        execution_payload,
        blob_versioned_hashes,
        parent_beacon_block_root,
    ))
    .unwrap();

    // if raw is set, print the raw payload, without quotes
    if args.raw {
        println!("{json_request}");
        return;
    }

    // construct the cast rpc command
    let mut prefix = "cast rpc".to_string();
    let suffix = format!("engine_newPayloadV3 --raw {json_request}");

    if let Some(rpc_url) = args.rpc_url {
        prefix += &format!(" --rpc-url {}", rpc_url);
    }

    if let Some(secret) = args.jwt_secret {
        prefix += &format!(" --jwt-secret {}", secret);
    }

    // add the suffix and request
    prefix += &format!(" {}", suffix);

    // print the payload
    println!("{prefix}");
}
