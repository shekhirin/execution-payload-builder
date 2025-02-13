use std::io::Read;

use alloy::{
    consensus::{
        Block as PrimitiveBlock, BlockBody, Header as PrimitiveHeader,
        Transaction as PrimitiveTransaction, TxEnvelope as EthTxEnvelope,
    },
    eips::{eip2930::AccessList, eip7702::SignedAuthorization, Encodable2718, Typed2718},
    primitives::{bytes::BufMut, Bytes, ChainId, TxKind, B256, U256},
    rpc::types::{
        engine::ExecutionPayload, Block as RpcBlock, BlockTransactions,
        Transaction as EthRpcTransaction,
    },
};
use clap::Parser;
use delegate::delegate;
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
    delegate! {
        to match self {
            RpcTransaction::Ethereum(tx) => tx,
            RpcTransaction::Optimism(tx) => tx,
        } {
            fn ty(&self) -> u8;
        }
    }
}

impl PrimitiveTransaction for RpcTransaction {
    delegate! {
        to match self {
            RpcTransaction::Ethereum(tx) => tx,
            RpcTransaction::Optimism(tx) => tx,
        } {
            fn chain_id(&self) -> Option<ChainId>;
            fn nonce(&self) -> u64;
            fn gas_limit(&self) -> u64;
            fn gas_price(&self) -> Option<u128>;
            fn max_fee_per_gas(&self) -> u128;
            fn max_priority_fee_per_gas(&self) -> Option<u128>;
            fn max_fee_per_blob_gas(&self) -> Option<u128>;
            fn priority_fee_or_price(&self) -> u128;
            fn effective_gas_price(&self, base_fee: Option<u64>) -> u128;
            fn is_dynamic_fee(&self) -> bool;
            fn kind(&self) -> TxKind;
            fn is_create(&self) -> bool;
            fn value(&self) -> U256;
            fn input(&self) -> &Bytes;
            fn access_list(&self) -> Option<&AccessList>;
            fn blob_versioned_hashes(&self) -> Option<&[B256]>;
            fn authorization_list(&self) -> Option<&[SignedAuthorization]>;
        }
    }
}

impl Encodable2718 for RpcTransaction {
    delegate! {
        to match self {
            RpcTransaction::Ethereum(tx) => tx.inner,
            RpcTransaction::Optimism(tx) => tx.inner.inner,
        } {
            fn encode_2718_len(&self) -> usize;
            fn encode_2718(&self, out: &mut dyn BufMut);
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
        prefix += &format!(" --rpc-url {rpc_url}");
    }

    if let Some(secret) = args.jwt_secret {
        prefix += &format!(" --jwt-secret {secret}");
    }

    // add the suffix and request
    prefix += &format!(" {suffix}");

    // print the payload
    println!("{prefix}");
}
