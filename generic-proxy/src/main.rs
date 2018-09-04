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

use std::sync::Arc;
use clap::App;

type Metadata = Option<Arc<::jsonrpc_pubsub::Session>>;
type Middleware<T> = (
    simple_cache::Middleware,
    upstream::Middleware<T>,
);

fn handler<T: upstream::Transport>(transport: T) -> rpc::MetaIoHandler<Metadata, Middleware<T>> {
    rpc::MetaIoHandler::with_middleware((
        simple_cache::Middleware::new(vec![
            simple_cache::Method::ignore_params("chain_getBlock")
        ]),
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
    let app = App::from_yaml(yml);
    // TODO [ToDr] Configure other app options]

    let _matches = app.get_matches_from(args);

    // Actually run the damn thing.
    let mut event_loop = tokio_core::reactor::Core::new().unwrap();
    let transport = upstream::ws::WebSocket::with_event_loop(
        "ws://localhost:9944",
        &event_loop.handle(),
    ).unwrap();


    let _server1 = transports::start_ws(vec![], handler(transport.clone())).unwrap();
    let _server2 = transports::start_http(vec![], handler(transport.clone())).unwrap();

    loop {
        event_loop.turn(None);
    }
}
