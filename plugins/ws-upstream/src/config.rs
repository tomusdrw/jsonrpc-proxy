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
//! WebSocket upstream configuration parameters.

use cli_params;

/// Configuration options of the WS upstream
pub enum Param {
    /// Upstream URL
    Url(url::Url),
}

/// Returns all configuration parameters for WS upstream.
pub fn params() -> Vec<cli_params::Param<Param>> {
    vec![cli_params::Param::new(
        "WebSockets upstream",
        "upstream-ws",
        "Address of the parent WebSockets RPC server that we should connect to.",
        "ws://127.0.0.1:9944",
        move |val: String| {
            let url = val
                .parse()
                .map_err(|e| format!("Invalid upstream address: {:?}", e))?;
            Ok(Param::Url(url))
        },
    )]
}
