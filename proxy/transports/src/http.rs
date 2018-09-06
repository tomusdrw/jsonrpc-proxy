//! HTTP server for the proxy.

use std::{
    io,
    net::{SocketAddr, Ipv4Addr},
    sync::Arc,
};

use jsonrpc_http_server as http;
use params::Param;
use pubsub;
use rpc;

const CATEGORY: &str = "HTTP Server";
const PREFIX: &str = "http";

/// Returns CLI configuration options for the HTTP server.
pub fn params<M, S>() -> Vec<Param<Box<Configurator<M, S>>>> where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    vec![
        param("port", "9934", "Configures HTTP server listening port.", |value| {
            let port: u16 = value.parse().map_err(|e| format!("Invalid port number {}: {}", value, e))?;
            Ok(move |address: &mut SocketAddr, builder| {
                address.set_port(port);
                Ok(builder)
            })
        }),
        param("ip", "127.0.0.1", "Configures HTTP server interface.", |value| {
            let ip: Ipv4Addr = value.parse().map_err(|e| format!("Invalid port number {}: {}", value, e))?;
            Ok(move |address: &mut SocketAddr, builder| {
                address.set_ip(ip.into());
                Ok(builder)
            })
        }),
        param("threads", "4", "Configures HTTP server threads.", |value| {
            let threads: usize = value.parse().map_err(|e| format!("Invalid threads number {}: {}", value, e))?;
            Ok(move |_address: &mut SocketAddr, builder: http::ServerBuilder<M, S>| {
                Ok(builder.threads(threads))
            })
        }),
    ]
}

/// Starts HTTP server on given handler.
pub fn start<T, M, S>(
    params: Vec<Box<Configurator<M, S>>>,
    io: T,
) -> io::Result<http::Server> where
    T: Into<rpc::MetaIoHandler<M, S>>,
    M: rpc::Metadata + Default + From<Option<Arc<pubsub::Session>>>,
    S: rpc::Middleware<M>,
{
    let mut builder = http::ServerBuilder::new(io);
    let mut address = "127.0.0.1:9934".parse().unwrap();

    // configure the server
    for p in params {
        builder = p.configure(&mut address, builder)?;
    }
    println!("HTTP listening on {}", address);

    builder.start_http(&address)
}

fn param<M, S, F, X>(name: &str, default_value: &str, description: &str, parser: F) -> Param<Box<Configurator<M, S>>> where
    F: Fn(String) -> Result<X, String> + 'static,
    X: Configurator<M, S> + 'static,
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    Param {
        category: CATEGORY.into(),
        name: format!("{}-{}", PREFIX, name),
        description: description.into(),
        default_value: default_value.into(),
        parser: Box::new(move |val: String| {
            Ok(Box::new(parser(val)?) as _)
        }),
    }
}

/// Configures the HTTP server.
pub trait Configurator<M, S> where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    /// Configure the server.
    fn configure(&self, address: &mut SocketAddr, builder: http::ServerBuilder<M, S>) -> io::Result<http::ServerBuilder<M, S>>;
}

impl<F, M, S> Configurator<M, S> for F where 
    F: Fn(&mut SocketAddr, http::ServerBuilder<M, S>) -> io::Result<http::ServerBuilder<M, S>>,
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    fn configure(&self, address: &mut SocketAddr, builder: http::ServerBuilder<M, S>) -> io::Result<http::ServerBuilder<M, S>> {
        (*self)(address, builder)
    }
}
