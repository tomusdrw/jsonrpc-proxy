#![warn(missing_docs)]

extern crate jsonrpc_core as rpc;
extern crate jsonrpc_http_server as http;
extern crate jsonrpc_pubsub as pubsub;
extern crate jsonrpc_ws_server as ws;

pub type RpcHandler = rpc::IoHandler;

/// Starts WebSockets server on given handler.
pub fn start_ws<T, M, S>(
    params: Vec<()>,
    io: T,
) -> ws::Result<ws::Server> where
    T: Into<rpc::MetaIoHandler<M, S>>,
    M: rpc::Metadata + Default,
    S: rpc::Middleware<M>,
{
    let mut builder = ws::ServerBuilder::new(io);
    let mut address = "127.0.0.1:9945".parse().unwrap();
    // configure the server
    for _p in params {
    
    }
    builder.start(&address)
}
