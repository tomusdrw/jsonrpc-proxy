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
//! Generic RPC proxy with default set of plugins.
//!
//! - Allows configuration to be passed via CLI options or a yaml file.
//! - Supports simple time-based cache

#![warn(missing_docs)]

use jsonrpc_core as rpc;

use clap::App;
use std::sync::Arc;

/// A generic proxy metadata.
pub type Metadata = Option<Arc<::jsonrpc_pubsub::Session>>;

type Middleware<T, E> = (
    permissioning::Middleware,
    simple_cache::Middleware,
    E,
    upstream::Middleware<T>,
);

fn handler<T: upstream::Transport, E: rpc::Middleware<Metadata>>(
    transport: T,
    extra: E,
    cache_params: &[simple_cache::config::Param],
    permissioning_params: &[permissioning::config::Param],
    upstream_params: &[upstream::config::Param],
) -> rpc::MetaIoHandler<Metadata, Middleware<T, E>> {
    rpc::MetaIoHandler::with_middleware((
        permissioning::Middleware::new(permissioning_params),
        simple_cache::Middleware::new(cache_params),
        extra,
        upstream::Middleware::new(transport, upstream_params),
    ))
}

/// TODO [ToDr] The whole thing is really shit.
pub trait Extension {
    /// Middleware type.
    type Middleware: rpc::Middleware<Metadata> + Clone;

    /// Configure clap application with parameters.
    fn configure_app<'a, 'b>(&'a mut self, app: clap::App<'a, 'b>) -> clap::App<'a, 'b>;

    /// Parse matches and create the middleware.
    fn parse_matches(matches: &clap::ArgMatches, upstream: impl upstream::Transport) -> Self::Middleware;
}

impl Extension for () {
    type Middleware = rpc::NoopMiddleware;

    fn configure_app<'a, 'b>(&'a mut self, app: clap::App<'a, 'b>) -> clap::App<'a, 'b> {
        app
    }

    fn parse_matches(_matches: &clap::ArgMatches, _upstream: impl upstream::Transport) -> Self::Middleware {
        Default::default()
    }
}

/// Run app with additional cache methods and upstream subscriptions.
pub fn run_app<E: Extension>(
    app: App,
    simple_cache_methods: Vec<simple_cache::Method>,
    upstream_subscriptions: Vec<upstream::Subscription>,
    mut extension: E,
) where
    <E::Middleware as rpc::Middleware<Metadata>>::Future: Unpin,
    <E::Middleware as rpc::Middleware<Metadata>>::CallFuture: Unpin,
{
    env_logger::init();
    let args = ::std::env::args_os();

    let ws_params = transports::ws::params();
    let app = cli::configure_app(app, &ws_params);
    let http_params = transports::http::params();
    let app = cli::configure_app(app, &http_params);
    let tcp_params = transports::tcp::params();
    let app = cli::configure_app(app, &tcp_params);
    let ipc_params = transports::ipc::params();
    let app = cli::configure_app(app, &ipc_params);

    let upstream_params = upstream::config::params();
    let app = cli::configure_app(app, &upstream_params);
    let ws_upstream_params = ws_upstream::config::params();
    let app = cli::configure_app(app, &ws_upstream_params);

    let cache_params = simple_cache::config::params();
    let app = cli::configure_app(app, &cache_params);

    let permissioning_params = permissioning::config::params();
    let app = cli::configure_app(app, &permissioning_params);

    let app = extension.configure_app(app);

    // Parse matches
    let matches = app.get_matches_from(args);
    let ws_params = cli::parse_matches(&matches, &ws_params).unwrap();
    let http_params = cli::parse_matches(&matches, &http_params).unwrap();
    let tcp_params = cli::parse_matches(&matches, &tcp_params).unwrap();
    let ipc_params = cli::parse_matches(&matches, &ipc_params).unwrap();
    let mut upstream_params = cli::parse_matches(&matches, &upstream_params).unwrap();
    upstream::config::add_subscriptions(&mut upstream_params, upstream_subscriptions);
    let ws_upstream_params = cli::parse_matches(&matches, &ws_upstream_params).unwrap();
    let mut cache_params = cli::parse_matches(&matches, &cache_params).unwrap();
    simple_cache::config::add_methods(&mut cache_params, simple_cache_methods);
    let permissioning_params = cli::parse_matches(&matches, &permissioning_params).unwrap();

    // Actually run the damn thing.
    let transport = ws_upstream::WebSocket::new(ws_upstream_params, |fut| std::mem::drop(tokio::spawn(fut))).unwrap();

    let extra = E::parse_matches(&matches, transport.clone());
    let h = || {
        handler(
            transport.clone(),
            extra.clone(),
            &cache_params,
            &permissioning_params,
            &upstream_params,
        )
    };
    let server1 = transports::ws::start(ws_params, h()).unwrap();
    let _server2 = transports::http::start(http_params, h()).unwrap();
    let _server3 = transports::tcp::start(tcp_params, h()).unwrap();
    let _server4 = transports::ipc::start(ipc_params, h()).unwrap();

    server1.wait().unwrap();
}
