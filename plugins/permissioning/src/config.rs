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
//! CLI configuration for permissioning.

use std::{fs, io};
use cli_params;
use serde_json;
use Permissioning;

/// A configuration option to apply.
pub enum Param {
    /// Permissioning configuration
    Config(Permissioning),
}

/// Returns a list of supported configuration parameters.
pub fn params() -> Vec<cli_params::Param<Param>> {
    vec![
        cli_params::Param::new(
            "Permissioning",
            "permissioning-config",
            "A path to a JSON file containing a list of methods that should be permissioned. See examples for the file schema.",
            "-",
            |path: String| {
                if &path == "-" {
                    return Ok(Param::Config(Default::default()))
                }

                let file = fs::File::open(&path).map_err(|e| format!("Can't open permissioning file at {}: {:?}", path, e))?;
                let buf_file = io::BufReader::new(file);
                let config: Permissioning = serde_json::from_reader(buf_file).map_err(|e| format!("Invalid JSON at {}: {:?}", path, e))?;
                Ok(Param::Config(config))
            }
        )
    ]
}
