//! Generic RPC proxy with default set of plugins.
//!
//! - Allows configuration to be passed via CLI options or a yaml file.
//! - Supports simple time-based cache

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

#[macro_use]
extern crate clap;
extern crate generic_proxy;

use clap::App;

fn main() {
    let yml = load_yaml!("./cli.yml");
    let app = App::from_yaml(yml).set_term_width(80);

    generic_proxy::run_app(app, vec![], vec![], ())
}
