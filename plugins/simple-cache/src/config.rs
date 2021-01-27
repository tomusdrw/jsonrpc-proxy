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
//! CLI configuration for simple cache.

use cli_params;
use serde_json;
use std::{fs, io};
use Method;

/// A configuration option to apply.
pub enum Param {
    /// Methods that should be cached.
    Config(Cache),
}

/// Returns a list of supported configuration parameters.
pub fn params() -> Vec<cli_params::Param<Param>> {
    vec![cli_params::Param::new(
        "Simple Cache",
        "simple-cache-config",
        "A path to a JSON file containing a list of methods that should be cached. See examples for the file schema.",
        "-",
        |path: String| {
            if &path == "-" {
                return Ok(Param::Config(Default::default()));
            }

            let file = fs::File::open(&path).map_err(|e| format!("Can't open cache file at {}: {:?}", path, e))?;
            let buf_file = io::BufReader::new(file);
            let methods: Cache =
                serde_json::from_reader(buf_file).map_err(|e| format!("Invalid JSON at {}: {:?}", path, e))?;
            Ok(Param::Config(methods))
        },
    )]
}

/// Add methods given as the first parameter to the config in one of the params.
pub fn add_methods(params: &mut [Param], methods: Vec<Method>) {
    for p in params {
        match p {
            Param::Config(ref mut config) => {
                config.methods.extend(methods.clone());
            }
        }
    }
}

/// Cache configuration
#[derive(Clone, Deserialize)]
pub struct Cache {
    /// If not enabled method definitions are ignored.
    pub enabled: bool,
    /// Per-method definitions
    pub methods: Vec<Method>,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            enabled: true,
            methods: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_deserialize_example() {
        let _m: Cache = serde_json::from_slice(include_bytes!("../../../examples/cache.json")).unwrap();
    }
}
