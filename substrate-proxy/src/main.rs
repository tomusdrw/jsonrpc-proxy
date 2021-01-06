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
//! JSON-RPC proxy suitable for Substrate nodes.
//!
//! The proxy contains a pre-configured list of cacheable methods and upstream subscriptions.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

#[macro_use]
extern crate clap;
extern crate generic_proxy;
extern crate simple_cache;
extern crate upstream;

#[tokio::main]
fn main() {
    let yml = load_yaml!("./cli.yml");
    let app = clap::App::from_yaml(yml).set_term_width(80);

    generic_proxy::run_app(
        app,
        vec![
            // author
            cache("author_pendingExtrinsics"),
            // chain
            cache("chain_getHeader"),
            cache("chain_getBlock"),
            cache("chain_getBlockHash"),
            cache("chain_getHead"),
            cache("chain_getRuntimeVersion"),
            // state
            cache("state_call"),
            cache("state_callAt"),
            cache("state_getStorage"),
            cache("state_getStorageAt"),
            cache("state_getStorageHash"),
            cache("state_getStorageHashAt"),
            cache("state_getStorageSize"),
            cache("state_getStorageSizeAt"),
            cache("state_queryStorage"),
            // system
            cache("system_name"),
            cache("system_version"),
            cache("system_chain"),
        ],
        vec![
            upstream::Subscription {
                subscribe: "author_submitAndWatchExtrinsic".into(),
                unsubscribe: "author_unwatchExtrinsic".into(),
                name: "author_extrinsicUpdate".into(),
            },
            upstream::Subscription {
                subscribe: "chain_subscribeNewHead".into(),
                unsubscribe: "chain_unsubscribeNewHead".into(),
                name: "chain_newHead".into(),
            },
            upstream::Subscription {
                subscribe: "state_subscribeStorage".into(),
                unsubscribe: "state_unsubscribeStorage".into(),
                name: "state_storage".into(),
            },
        ],
        ()
    )
}

fn cache(name: &str) -> simple_cache::Method {
    simple_cache::Method::new(
        name, 
        simple_cache::CacheEviction::Time(::std::time::Duration::from_secs(3)),
    )
}


