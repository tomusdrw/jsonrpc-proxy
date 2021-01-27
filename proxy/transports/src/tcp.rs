// Copyright (c) 2018-2020 jsonrpc-proxy contributors.
//
// This file is part of jsonrpc-proxy
// (see https://github.com/tomusdrw/jsonrpc-proxy).
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.
//! TCP server for the proxy.

use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use jsonrpc_tcp_server as tcp;
use params::Param;
use pubsub;
use rpc;

const CATEGORY: &str = "TCP Server";
const PREFIX: &str = "tcp";

/// Returns CLI configuration options for the TCP server.
pub fn params<M, S>() -> Vec<Param<Box<dyn Configurator<M, S>>>>
where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
    S::Future: Unpin,
    S::CallFuture: Unpin,
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
pub fn start<T, M, S>(params: Vec<Box<dyn Configurator<M, S>>>, io: T) -> io::Result<tcp::Server>
where
    T: Into<rpc::MetaIoHandler<M, S>>,
    M: rpc::Metadata + Default + From<Option<Arc<pubsub::Session>>>,
    S: rpc::Middleware<M>,
    S::Future: Unpin,
    S::CallFuture: Unpin,
{
    let mut builder =
        tcp::ServerBuilder::with_meta_extractor(io, |context: &tcp::RequestContext| {
            Some(Arc::new(pubsub::Session::new(context.sender.clone()))).into()
        });
    // should be overwritten by parameters anyway
    let mut address = "127.0.0.1:9955".parse().unwrap();
    // configure the server
    for p in params {
        builder = p.configure(&mut address, builder)?;
    }

    println!("TCP listening on {}", address);

    builder.start(&address)
}

/// Configures the TCP server.
pub trait Configurator<M, S>
where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    /// Configure the server.
    fn configure(
        &self,
        address: &mut SocketAddr,
        builder: tcp::ServerBuilder<M, S>,
    ) -> io::Result<tcp::ServerBuilder<M, S>>;
}

impl<F, M, S> Configurator<M, S> for F
where
    F: Fn(&mut SocketAddr, tcp::ServerBuilder<M, S>) -> io::Result<tcp::ServerBuilder<M, S>>,
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    fn configure(
        &self,
        address: &mut SocketAddr,
        builder: tcp::ServerBuilder<M, S>,
    ) -> io::Result<tcp::ServerBuilder<M, S>> {
        (*self)(address, builder)
    }
}

fn param<M, S, F, X>(
    name: &str,
    default_value: &str,
    description: &str,
    parser: F,
) -> Param<Box<dyn Configurator<M, S>>>
where
    F: Fn(String) -> Result<X, String> + 'static,
    X: Configurator<M, S> + 'static,
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
    S::Future: Unpin,
    S::CallFuture: Unpin,
{
    Param {
        category: CATEGORY.into(),
        name: format!("{}-{}", PREFIX, name),
        description: description.replace('\n', " "),
        default_value: default_value.into(),
        parser: Box::new(move |val: String| Ok(Box::new(parser(val)?) as _)),
    }
}
