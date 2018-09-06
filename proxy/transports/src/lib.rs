//! RPC Proxy servers

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate cli_params as params;
extern crate jsonrpc_core as rpc;
extern crate jsonrpc_pubsub as pubsub;

extern crate jsonrpc_http_server;
extern crate jsonrpc_ws_server;

pub mod ws;
pub mod http;
