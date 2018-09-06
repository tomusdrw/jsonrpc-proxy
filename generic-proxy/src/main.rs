//! Generic RPC proxy with default set of plugins.
//!
//! - Allows configuration to be passed via CLI options or a yaml file.
//! - Supports simple time-based cache

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

#[macro_use]
extern crate clap;

extern crate cli;
extern crate env_logger;
extern crate jsonrpc_core as rpc;
extern crate jsonrpc_pubsub;
extern crate tokio_core;

extern crate simple_cache;
extern crate transports;
extern crate upstream;
extern crate ws_upstream;

use std::sync::Arc;
use clap::App;

type Metadata = Option<Arc<::jsonrpc_pubsub::Session>>;
type Middleware<T> = (
    simple_cache::Middleware,
    upstream::Middleware<T>,
);

fn handler<T: upstream::Transport>(transport: T, cache_params: &[simple_cache::config::Param]) -> rpc::MetaIoHandler<Metadata, Middleware<T>> {
    rpc::MetaIoHandler::with_middleware((
        simple_cache::Middleware::new(cache_params),
        upstream::Middleware::new(transport, vec![upstream::Subscription {
            subscribe: "state_subscribeStorage".into(),
            unsubscribe: "state_unsubscribeStorage".into(),
            name: "state_storage".into(),
        }]),
    ))
}

fn main() {
    env_logger::init();
    let args = ::std::env::args_os();

    let yml = load_yaml!("./cli.yml");
    let app = App::from_yaml(yml).set_term_width(80);

    // TODO [ToDr] Configure other app options]
    let ws_params = transports::ws::params();
    let http_params = transports::http::params();
    let app = cli::configure_app(app, &ws_params);
    let app = cli::configure_app(app, &http_params);

    let upstream_params = ws_upstream::config::params();
    let app = cli::configure_app(app, &upstream_params);

    let cache_params = simple_cache::config::params();
    let app = cli::configure_app(app, &cache_params);

    // Parse matches
    let matches = app.get_matches_from(args);
    let ws_params = cli::parse_matches(&matches, &ws_params).unwrap();
    let http_params = cli::parse_matches(&matches, &http_params).unwrap();
    let upstream_params = cli::parse_matches(&matches, &upstream_params).unwrap();
    let cache_params = cli::parse_matches(&matches, &cache_params).unwrap();

    // Actually run the damn thing.
    let mut event_loop = tokio_core::reactor::Core::new().unwrap();
    let transport = ws_upstream::WebSocket::new(
        &event_loop.handle(),
        upstream_params,
    ).unwrap();


    let _server1 = transports::ws::start(ws_params, handler(transport.clone(), &cache_params)).unwrap();
    let _server2 = transports::http::start(http_params, handler(transport.clone(), &cache_params)).unwrap();

    loop {
        event_loop.turn(None);
    }
}
