#![warn(missing_docs)]

#[macro_use]
extern crate clap;

extern crate cli;
extern crate env_logger;
extern crate proxy;
extern crate tokio_core;
extern crate transports;

use clap::{App, Arg};

fn main() {
    env_logger::init();
    let args = ::std::env::args_os();

    let yml = load_yaml!("./cli.yml");
    let mut app = App::from_yaml(yml);
    // TODO [ToDr] Configure other app options]

    let matches = app.get_matches_from(args);

    // Actually run the damn thing.
    let mut event_loop = tokio_core::reactor::Core::new().unwrap();
    let transport = proxy::passthrough::ws::WebSocket::with_event_loop(
        "ws://localhost:9944",
        &event_loop.handle(),
    ).unwrap();

    let handler = proxy::handler(transport);
    let server = transports::start_ws(vec![], handler).unwrap();

    loop {
        event_loop.turn(None);
    }

    server.wait().unwrap();
}
