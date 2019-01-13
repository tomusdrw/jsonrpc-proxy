//! Generic RPC proxy with default set of plugins.
//!
//! - Allows configuration to be passed via CLI options or a yaml file.
//! - Supports simple time-based cache

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate clap;
extern crate cli;
extern crate env_logger;
extern crate jsonrpc_core as rpc;
extern crate jsonrpc_pubsub;

extern crate permissioning;
extern crate simple_cache;
extern crate transports;
extern crate upstream;
extern crate ws_upstream;
extern crate ipc_upstream;

extern crate tokio;

use std::sync::Arc;
use clap::App;
use rpc::futures::Future;
use tokio::runtime::Runtime;

type Metadata = Option<Arc<::jsonrpc_pubsub::Session>>;
type Middleware<T> = (
    permissioning::Middleware,
    simple_cache::Middleware,
    upstream::Middleware<T>,
);

fn handler<T: upstream::Transport>(
    transport: T,
    cache_params: &[simple_cache::config::Param],
    permissioning_params: &[permissioning::config::Param],
    upstream_params: &[upstream::config::Param],
) -> rpc::MetaIoHandler<Metadata, Middleware<T>> {
    rpc::MetaIoHandler::with_middleware((
        permissioning::Middleware::new(permissioning_params),
        simple_cache::Middleware::new(cache_params),
        upstream::Middleware::new(transport, upstream_params),
    ))
}

/// Run app with additional cache methods and upstream subscriptions.
pub fn run_app(
    app: App,
    simple_cache_methods: Vec<simple_cache::Method>,
    upstream_subscriptions: Vec<upstream::Subscription>,
) {
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
    let ipc_upstream_params = ipc_upstream::config::params();
    let app = cli::configure_app(app, &ipc_upstream_params);

    let cache_params = simple_cache::config::params();
    let app = cli::configure_app(app, &cache_params);

    let permissioning_params = permissioning::config::params();
    let app = cli::configure_app(app, &permissioning_params);

    // Parse matches
    let matches = app.get_matches_from(args);
    let ws_params = cli::parse_matches(&matches, &ws_params).unwrap();
    let http_params = cli::parse_matches(&matches, &http_params).unwrap();
    let tcp_params = cli::parse_matches(&matches, &tcp_params).unwrap();
    let ipc_params = cli::parse_matches(&matches, &ipc_params).unwrap();
    let mut upstream_params = cli::parse_matches(&matches, &upstream_params).unwrap();
    upstream::config::add_subscriptions(&mut upstream_params, upstream_subscriptions);
    let ws_upstream_params = cli::parse_matches(&matches, &ws_upstream_params).unwrap();
    let ipc_upstream_params = cli::parse_matches(&matches, &ipc_upstream_params).unwrap();
    let mut cache_params = cli::parse_matches(&matches, &cache_params).unwrap();
    simple_cache::config::add_methods(&mut cache_params, simple_cache_methods);
    let permissioning_params = cli::parse_matches(&matches, &permissioning_params).unwrap();

    // Actually run the damn thing.
    let mut runtime = Runtime::new().unwrap();

    // let transport = ws_upstream::WebSocket::new(
    //     &mut runtime,
    //     ws_upstream_params,
    // ).unwrap();

    let transport = ipc_upstream::IPC::new(
        &mut runtime,
        ipc_upstream_params,
    ).unwrap();

    let h = || handler(transport.clone(), &cache_params, &permissioning_params, &upstream_params);
    let _server1 = transports::ws::start(ws_params, h()).unwrap();
    let _server2 = transports::http::start(http_params, h()).unwrap();
    let _server3 = transports::tcp::start(tcp_params, h()).unwrap();
    let _server4 = transports::ipc::start(ipc_params, h()).unwrap();

    runtime.shutdown_on_idle().wait().unwrap();
}
