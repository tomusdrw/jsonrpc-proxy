//! TCP server for the proxy.

use std::{
    io,
    net::{SocketAddr, Ipv4Addr},
    sync::Arc,
};

use jsonrpc_tcp_server as tcp;
use params::Param;
use pubsub;
use rpc;

const CATEGORY: &str = "TCP Server";
const PREFIX: &str = "tcp";

/// Returns CLI configuration options for the TCP server.
pub fn params<M, S>() -> Vec<Param<Box<Configurator<M, S>>>> where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    vec![
        param("port", "9955", "Configures TCP server listening port.", |value| {
            let port: u16 = value.parse().map_err(|e| format!("Invalid port number {}: {}", value, e))?;
            Ok(move |address: &mut SocketAddr, builder| {
                address.set_port(port);
                Ok(builder)
            })
        }),
        param("ip", "127.0.0.1", "Configures TCP server interface.", |value| {
            let ip: Ipv4Addr = value.parse().map_err(|e| format!("Invalid port number {}: {}", value, e))?;
            Ok(move |address: &mut SocketAddr, builder| {
                address.set_ip(ip.into());
                Ok(builder)
            })
        }),
        param("request-separator", "10",
            "Configures TCP server request separator (single byte). If \"none\" the parser will try to figure out requests boundaries. Default is new line character.",
            |value| {
                let separator = match value.as_str() {
                    "none" => tcp::Separator::Empty,
                    _ => tcp::Separator::Byte(value.parse().map_err(|e| format!("Invalid separator code {}: {}", value, e))?),
                };
                Ok(move |_address: &mut SocketAddr, builder: tcp::ServerBuilder<M, S>| {
                    Ok(builder.request_separators(separator.clone(), separator.clone()))
                })
            }
        ),
    ]
}
 
/// Starts TCP server on given handler.
pub fn start<T, M, S>(
    params: Vec<Box<Configurator<M, S>>>,
    io: T,
) -> io::Result<tcp::Server> where
    T: Into<rpc::MetaIoHandler<M, S>>,
    M: rpc::Metadata + Default + From<Option<Arc<pubsub::Session>>>,
    S: rpc::Middleware<M>,
{
    let mut builder = tcp::ServerBuilder::with_meta_extractor(io, |context: &tcp::RequestContext| {
        Some(Arc::new(pubsub::Session::new(context.sender.clone()))).into()
    });
    // should be overwritten by parameters anyway
    let mut address = "127.0.0.1:9945".parse().unwrap();
    // configure the server
    for p in params {
        builder = p.configure(&mut address, builder)?;
    }

    println!("TCP listening on {}", address);

    builder.start(&address)
}

/// Configures the TCP server.
pub trait Configurator<M, S> where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    /// Configure the server.
    fn configure(&self, address: &mut SocketAddr, builder: tcp::ServerBuilder<M, S>) -> io::Result<tcp::ServerBuilder<M, S>>;
}

impl<F, M, S> Configurator<M, S> for F where 
    F: Fn(&mut SocketAddr, tcp::ServerBuilder<M, S>) -> io::Result<tcp::ServerBuilder<M, S>>,
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    fn configure(&self, address: &mut SocketAddr, builder: tcp::ServerBuilder<M, S>) -> io::Result<tcp::ServerBuilder<M, S>> {
        (*self)(address, builder)
    }
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
        description: description.replace('\n', " "),
        default_value: default_value.into(),
        parser: Box::new(move |val: String| {
            Ok(Box::new(parser(val)?) as _)
        }),
    }
}
