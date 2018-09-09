//! JSON-RPC proxy suitable for Ethereum nodes.
//!
//! The proxy contains a pre-configured list of cacheable methods and upstream subscriptions.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

#[macro_use]
extern crate clap;
extern crate generic_proxy;
extern crate simple_cache;
extern crate upstream;

fn main() {
    let yml = load_yaml!("./cli.yml");
    let app = clap::App::from_yaml(yml).set_term_width(80);

    generic_proxy::run_app(
        app,
        vec![
            // eth
            cache("eth_protocolVersion"),
            cache("eth_syncing"),
            cache("eth_mining"),
            cache("eth_gasPrice"),
            cache("eth_blockNumber"),
            cache("eth_getBalance"),
            cache("eth_getStorageAt"),
            cache("eth_getBlockByHash"),
            cache("eth_getBlockByNumber"),
            cache("eth_getTransactionCount"),
            cache("eth_getBlockTransactionCountByHash"),
            cache("eth_getBlockTransactionCountByNumber"),
            cache("eth_getUncleCountByBlockHash"),
            cache("eth_getUncleCountByBlockNumber"),
            cache("eth_getCode"),
            cache("eth_call"),
            cache("eth_estimateGas"),
            cache("eth_getTransactionByHash"),
            cache("eth_getTransactionByBlockHashAndIndex"),
            cache("eth_getTransactionByBlockNumberAndIndex"),
            cache("eth_getTransactionReceipt"),
            cache("eth_getUncleByBlockHashAndIndex"),
            cache("eth_getUncleByBlockNumberAndIndex"),
            cache("eth_getCompilers"),
            cache("eth_getLogs"),
            // net
            cache("net_version"),
            cache("net_peerCount"),
            cache("net_listening"),
            // parity
            cache("parity_transactionsLimit"),
            cache("parity_extraData"),
            cache("parity_gasFloorTarget"),
            cache("parity_gasCeilTarget"),
            cache("parity_minGasPrice"),
            cache("parity_netChain"),
            cache("parity_netPort"),
            cache("parity_rpcSettings"),
            cache("parity_nodeName"),
            cache("parity_defaultExtraData"),
            cache("parity_gasPriceHistogram"),
            cache("parity_phraseToAddress"),
            cache("parity_registryAddress"),
            cache("parity_wsUrl"),
            cache("parity_chainId"),
            cache("parity_chain"),
            cache("parity_enode"),
            cache("parity_versionInfo"),
            cache("parity_releaseInfo"),
            cache("parity_chainStatus"),
            cache("parity_getBlockHeaderByNumber"),
            cache("parity_cidV0"),
            // web3
            cache("web3_clientVersion"),
            cache("web3_sha3"),
        ],
        vec![
            upstream::Subscription {
                subscribe: "eth_subscribe".into(),
                unsubscribe: "eth_unsubscribe".into(),
                name: "eth_subscription".into(),
            },
            upstream::Subscription {
                subscribe: "parity_subscribe".into(),
                unsubscribe: "parity_unsubscribe".into(),
                name: "parity_subscription".into(),
            },
            upstream::Subscription {
                subscribe: "signer_subscribePending".into(),
                unsubscribe: "signer_unsubscribePending".into(),
                name: "signer_pending".into(),
            },
        ]
    )
}

fn cache(name: &str) -> simple_cache::Method {
    simple_cache::Method::new(
        name, 
        simple_cache::CacheEviction::Time(::std::time::Duration::from_secs(3)),
    )
}
