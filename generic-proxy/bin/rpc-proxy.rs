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

#[tokio::main]
async fn main() {
    let yml = clap::load_yaml!("./cli.yml");
    let app = clap::App::from_yaml(yml).set_term_width(80);

    generic_proxy::run_app(app, vec![], vec![], ())
}
