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
//! WebSockets server for the proxy.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use jsonrpc_ws_server as ws;
use params::Param;
use pubsub;
use rpc;

const CATEGORY: &str = "WebSockets Server";
const PREFIX: &str = "websockets";

/// Returns CLI configuration options for the WS server.
pub fn params<M, S>() -> Vec<Param<Box<dyn Configurator<M, S>>>>
where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
    S::Future: Unpin,
    S::CallFuture: Unpin,
{
    vec![
        param(
            "port",
            "9945",
            "Configures WebSockets server listening port.",
            |value| {
                let port: u16 = value
                    .parse()
                    .map_err(|e| format!("Invalid port number {}: {}", value, e))?;
                Ok(move |address: &mut SocketAddr, builder| {
                    address.set_port(port);
                    Ok(builder)
                })
            },
        ),
        param(
            "ip",
            "127.0.0.1",
            "Configures WebSockets server interface.",
            |value| {
                let ip: Ipv4Addr = value
                    .parse()
                    .map_err(|e| format!("Invalid port number {}: {}", value, e))?;
                Ok(move |address: &mut SocketAddr, builder| {
                    address.set_ip(ip.into());
                    Ok(builder)
                })
            },
        ),
        param(
            "hosts",
            "none",
            r#"
List of allowed Host header values. This option will
validate the Host header sent by the browser, it is
additional security against some attack vectors. Special
options: "all", "none"."#,
            |value| {
                let hosts = match value.as_str() {
                    "none" => Some(vec![]),
                    "*" | "all" | "any" => None,
                    _ => Some(value.split(',').map(Into::into).collect()),
                };
                Ok(
                    move |_address: &mut SocketAddr, builder: ws::ServerBuilder<M, S>| {
                        Ok(builder.allowed_hosts(hosts.clone().into()))
                    },
                )
            },
        ),
        param(
            "origins",
            "none",
            r#"
Specify Origin header values allowed to connect. Special
options: "all", "none". "#,
            |value| {
                let origins = match value.as_str() {
                    "none" => Some(vec![]),
                    "*" | "all" | "any" => None,
                    _ => Some(value.split(',').map(Into::into).collect()),
                };

                Ok(
                    move |_address: &mut SocketAddr, builder: ws::ServerBuilder<M, S>| {
                        Ok(builder.allowed_origins(origins.clone().into()))
                    },
                )
            },
        ),
        param(
            "max-connections",
            "100",
            "Maximum number of allowed concurrent WebSockets JSON-RPC connections.",
            |value| {
                let max_connections: usize = value
                    .parse()
                    .map_err(|e| format!("Invalid number of connections {}: {}", value, e))?;
                Ok(
                    move |_address: &mut SocketAddr, builder: ws::ServerBuilder<M, S>| {
                        Ok(builder.max_connections(max_connections))
                    },
                )
            },
        ),
    ]
}

/// Starts WebSockets server on given handler.
pub fn start<T, M, S>(params: Vec<Box<dyn Configurator<M, S>>>, io: T) -> ws::Result<ws::Server>
where
    T: Into<rpc::MetaIoHandler<M, S>>,
    M: rpc::Metadata + Default + From<Option<Arc<pubsub::Session>>>,
    S: rpc::Middleware<M>,
    S::Future: Unpin,
    S::CallFuture: Unpin,
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
pub trait Configurator<M, S>
where
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    /// Configure the server.
    fn configure(
        &self,
        address: &mut SocketAddr,
        builder: ws::ServerBuilder<M, S>,
    ) -> ws::Result<ws::ServerBuilder<M, S>>;
}

impl<F, M, S> Configurator<M, S> for F
where
    F: Fn(&mut SocketAddr, ws::ServerBuilder<M, S>) -> ws::Result<ws::ServerBuilder<M, S>>,
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    fn configure(
        &self,
        address: &mut SocketAddr,
        builder: ws::ServerBuilder<M, S>,
    ) -> ws::Result<ws::ServerBuilder<M, S>> {
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
