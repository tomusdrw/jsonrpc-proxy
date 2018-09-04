#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate jsonrpc_core as rpc;
extern crate jsonrpc_http_server as http;
extern crate jsonrpc_pubsub as pubsub;
extern crate jsonrpc_ws_server as ws;

use std::{
    io,
    sync::Arc
};

/// Starts WebSockets server on given handler.
pub fn start_ws<T, M, S>(
    params: Vec<()>,
    io: T,
) -> ws::Result<ws::Server> where
    T: Into<rpc::MetaIoHandler<M, S>>,
    M: rpc::Metadata + Default + From<Option<Arc<pubsub::Session>>>,
    S: rpc::Middleware<M>,
{
    let builder = ws::ServerBuilder::with_meta_extractor(io, |context: &ws::RequestContext| {
        Some(Arc::new(pubsub::Session::new(context.sender()))).into()
    });
    let address = "127.0.0.1:9945".parse().unwrap();
    // configure the server
    for _p in params {
    
    }
    println!("WS listening on {}", address);

    builder.start(&address)
}

/// Starts HTTP server on given handler.
pub fn start_http<T, M, S>(
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
