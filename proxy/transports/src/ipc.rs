//! IPC server for the proxy.

use std::{
    io,
    sync::Arc,
};

use jsonrpc_ipc_server as ipc;
use params::Param;
use pubsub;
use rpc;

const CATEGORY: &str = "IPC Server";
const PREFIX: &str = "ipc";

/// Returns CLI configuration options for the IPC server.
pub fn params<M, S>() -> Vec<Param<Box<dyn Configurator<M, S>>>> where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    vec![
        param("path", "./jsonrpc.ipc", "Configures IPC server socket path.", |value| {
            Ok(move |path: &mut String, builder| {
                *path = value.clone();
                Ok(builder)
            })
        }),
        param("request-separator", "none",
            "Configures TCP server request separator (single byte). If \"none\" the parser will try to figure out requests boundaries.",
            |value| {
                let separator = match value.as_str() {
                    "none" => ipc::Separator::Empty,
                    _ => ipc::Separator::Byte(value.parse().map_err(|e| format!("Invalid separator code {}: {}", value, e))?),
                };
                Ok(move |_path: &mut String, builder: ipc::ServerBuilder<M, S>| {
                    Ok(builder.request_separators(separator.clone(), separator.clone()))
                })
            }
        ),
    ]
}
 
/// Starts IPC server on given handler.
pub fn start<T, M, S>(
    params: Vec<Box<dyn Configurator<M, S>>>,
    io: T,
) -> io::Result<ipc::Server> where
    T: Into<rpc::MetaIoHandler<M, S>>,
    M: rpc::Metadata + Default + From<Option<Arc<pubsub::Session>>>,
    S: rpc::Middleware<M>,
{
    let mut builder = ipc::ServerBuilder::with_meta_extractor(io, |context: &ipc::RequestContext| {
        Some(Arc::new(pubsub::Session::new(context.sender.clone()))).into()
    });
    // should be overwritten by parameters anyway
    let mut path = "./jsonrpc.ipc".to_owned();
    // configure the server
    for p in params {
        builder = p.configure(&mut path, builder)?;
    }

    println!("IPC listening at {}", path);

    builder.start(&path)
}

/// Configures the IPC server.
pub trait Configurator<M, S> where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    /// Configure the server.
    fn configure(&self, path: &mut String, builder: ipc::ServerBuilder<M, S>) -> io::Result<ipc::ServerBuilder<M, S>>;
}

impl<F, M, S> Configurator<M, S> for F where 
    F: Fn(&mut String, ipc::ServerBuilder<M, S>) -> io::Result<ipc::ServerBuilder<M, S>>,
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    fn configure(&self, path: &mut String, builder: ipc::ServerBuilder<M, S>) -> io::Result<ipc::ServerBuilder<M, S>> {
        (*self)(path, builder)
    }
}

fn param<M, S, F, X>(name: &str, default_value: &str, description: &str, parser: F) -> Param<Box<dyn Configurator<M, S>>> where
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
