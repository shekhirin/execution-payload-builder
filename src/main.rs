use std::io::Read;

use alloy::{
    consensus::{
        Block as PrimitiveBlock, BlockBody, Header as PrimitiveHeader,
        Transaction as PrimitiveTransaction, TxEnvelope as EthTxEnvelope,
    },
    network::AnyTxEnvelope,
    rpc::types::{engine::ExecutionPayload, Block as RpcBlock, BlockTransactions},
};
use clap::Parser;

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

fn main() -> eyre::Result<()> {
    let args = Args::parse();

    // read the file specified in `--path` otherwise read from stdin
    let block_json = if let Some(path) = &args.path {
        std::fs::read_to_string(path)?
    } else {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        buffer
    };

    // parse the input
    let block: RpcBlock<AnyTxEnvelope> = serde_json::from_str(&block_json)?;

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
            if let AnyTxEnvelope::Ethereum(EthTxEnvelope::Eip4844(tx)) = tx {
                return tx.tx().blob_versioned_hashes().map(|v| v.to_vec());
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
    ))?;

    if args.raw {
        // if raw is set, print the raw payload
        println!("{json_request}");
    } else {
        // otherwise, construct the cast rpc command
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

    Ok(())
}
