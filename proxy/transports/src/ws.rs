//! WebSockets server for the proxy.

use std::{
    net::{SocketAddr, Ipv4Addr},
    sync::Arc,
};

use params::Param;
use pubsub;
use rpc;
use jsonrpc_ws_server as ws;

const CATEGORY: &str = "WebSockets Server";
const PREFIX: &str = "websockets";

pub fn params<M, S>() -> Vec<Param<Box<Configurator<M, S>>>> where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    vec![
        param("port", "9945", "Configures WebSockets server listening port.", |value| {
            let port: u16 = value.parse().map_err(|e| format!("Invalid port number {}: {}", value, e))?;
            Ok(move |address: &mut SocketAddr, builder| {
                address.set_port(port);
                Ok(builder)
            })
        }),
        param("ip", "127.0.0.1", "Configures WebSockets server interface.", |value| {
            let ip: Ipv4Addr = value.parse().map_err(|e| format!("Invalid port number {}: {}", value, e))?;
            Ok(move |address: &mut SocketAddr, builder| {
                address.set_ip(ip.into());
                Ok(builder)
            })
        })
    ]
}
 
/// Starts WebSockets server on given handler.
pub fn start<T, M, S>(
    params: Vec<Box<Configurator<M, S>>>,
    io: T,
) -> ws::Result<ws::Server> where
    T: Into<rpc::MetaIoHandler<M, S>>,
    M: rpc::Metadata + Default + From<Option<Arc<pubsub::Session>>>,
    S: rpc::Middleware<M>,
{
    let mut builder = ws::ServerBuilder::with_meta_extractor(io, |context: &ws::RequestContext| {
        Some(Arc::new(pubsub::Session::new(context.sender()))).into()
    });
    // should be overwritten by parameters anyway
    let mut address = "127.0.0.1:9945".parse().unwrap();
    // configure the server
    for p in params {
        builder = p.configure(&mut address, builder)?;
    }

    println!("WS listening on {}", address);

    builder.start(&address)
}

/// Configures the WS server.
pub trait Configurator<M, S> where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    /// Configure the server.
    fn configure(&self, address: &mut SocketAddr, builder: ws::ServerBuilder<M, S>) -> ws::Result<ws::ServerBuilder<M, S>>;
}

impl<F, M, S> Configurator<M, S> for F where 
    F: Fn(&mut SocketAddr, ws::ServerBuilder<M, S>) -> ws::Result<ws::ServerBuilder<M, S>>,
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    fn configure(&self, address: &mut SocketAddr, builder: ws::ServerBuilder<M, S>) -> ws::Result<ws::ServerBuilder<M, S>> {
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
        arg: name.into(),
        name: format!("{}-{}", PREFIX, name),
        description: description.into(),
        default_value: default_value.into(),
        parser: Box::new(move |val: String| {
            Ok(Box::new(parser(val)?) as _)
        }),
    }
}
