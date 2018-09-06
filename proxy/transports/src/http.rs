//! HTTP server for the proxy.

use std::{
    io,
    sync::Arc,
};

use rpc;
use pubsub;
use jsonrpc_http_server as http;

/// Starts HTTP server on given handler.
pub fn start<T, M, S>(
    params: Vec<()>,
    io: T,
) -> io::Result<http::Server> where
    T: Into<rpc::MetaIoHandler<M, S>>,
    M: rpc::Metadata + Default + From<Option<Arc<pubsub::Session>>>,
    S: rpc::Middleware<M>,
{
    let builder = http::ServerBuilder::new(io);
    let address = "127.0.0.1:9934".parse().unwrap();
    // configure the server
    for _p in params {
    
    }
    println!("HTTP listening on {}", address);

    builder.threads(4).start_http(&address)
}
