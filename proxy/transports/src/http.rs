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
pub fn params<M, S>() -> Vec<Param<Box<dyn Configurator<M, S>>>> where
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
        param("rest-api", "disabled", r#"
Enables REST -> RPC converter for HTTP server. Allows you to
call RPC methods with `POST /<methodname>/<param1>/<param2>`.
The "secure" option requires the `Content-Type: application/json`
header to be sent with the request (even though the payload is ignored)
to prevent accepting POST requests from any website (via form submission).
The "unsecure" option does not require any `Content-Type`.
Possible options: "unsecure", "secure", "disabled"."#,
            |value| {
                let api = match value.as_str() {
                    "disabled" | "off" | "no" => http::RestApi::Disabled,
                    "secure" | "on" | "yes" => http::RestApi::Secure,
                    "unsecure" => http::RestApi::Unsecure,
                    _ => return Err(format!("Invalid value for rest-api: {}", value)),
                };
                Ok(move |_address: &mut SocketAddr, builder: http::ServerBuilder<M, S>| {
                    Ok(builder.rest_api(api))
                })
            }
        ),
        param("hosts", "none", r#"
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
                Ok(move |_address: &mut SocketAddr, builder: http::ServerBuilder<M, S>| {
                    Ok(builder.allowed_hosts(hosts.clone().into()))
                })
            }
        ),
        param("cors", "none", r#"
Specify CORS header for HTTP JSON-RPC API responses.
Special options: "all", "null", "none"."#,
            |value| {
                let cors = match value.as_str() {
                    "none" => Some(vec![]),
                    "*" | "all" | "any" => None,
                    _ => Some(value.split(',').map(Into::into).collect()),
                };

                Ok(move |_address: &mut SocketAddr, builder: http::ServerBuilder<M, S>| {
                    Ok(builder.cors(cors.clone().into()))
                })
            }
        ),
        param("cors-max-age", "3600000", r#"Configures AccessControlMaxAge header value in milliseconds.
Informs the client that the preflight request is not required for the specified time. Use 0 to disable."#,
            |value| {
                let cors_max_age: u32 = value.parse().map_err(|e| format!("Invalid cors max age {}: {}", value, e))?;

                Ok(move |_address: &mut SocketAddr, builder: http::ServerBuilder<M, S>| {
                    Ok(builder.cors_max_age(if cors_max_age == 0 {
                        None
                    } else {
                        Some(cors_max_age)
                    }))
                })
            }
        ),
        param("max-payload", "5", "Maximal HTTP server payload in Megabytes.",
            |value| {
                let max_payload: usize = value.parse().map_err(|e| format!("Invalid maximal payload size ({}): {}", value, e))?;
                Ok(move |_address: &mut SocketAddr, builder: http::ServerBuilder<M, S>| {
                    Ok(builder.max_request_body_size(max_payload * 1024 * 1024))
                })
            }
        ),
    ]
}

/// Starts HTTP server on given handler.
pub fn start<T, M, S>(
    params: Vec<Box<dyn Configurator<M, S>>>,
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

fn param<M, S, F, X>(name: &str, default_value: &str, description: &str, parser: F) -> Param<Box<dyn Configurator<M, S>>> where
    F: Fn(String) -> Result<X, String> + 'static,
    X: Configurator<M, S> + 'static,
    M: rpc::Metadata,
    S: rpc::Middleware<M>,
{
    Param {
        category: CATEGORY.into(),
        name: format!("{}-{}", PREFIX, name),
        description: description.replace('\n', ""),
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
