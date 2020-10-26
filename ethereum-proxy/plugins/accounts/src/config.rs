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
//! CLI configuration for accounts.

use cli_params;
use ethsign::{Protected, KeyFile};

/// A configuration option to apply.
pub enum Param {
    /// Account keyfile.
    Account(Option<KeyFile>),
    /// Password to the keyfile.
    Pass(Protected),
}

/// Returns a list of supported configuration parameters.
pub fn params() -> Vec<cli_params::Param<Param>> {
    vec![
        cli_params::Param::new(
            "Password to the keyfile",
            "account-password",
            "A password to unlock the keyfile.",
            "",
            |pass: String| {
                Ok(Param::Pass(pass.into()))
            }
        ),
        cli_params::Param::new(
            "Account to unlock",
            "account-file",
            "A path to a JSON wallet with the account.",
            "-",
            |path: String| {
                if path == "-" {
                    return Ok(Param::Account(None))
                }

                let file = std::fs::File::open(path).map_err(to_str)?;
                let key: KeyFile = serde_json::from_reader(file).map_err(to_str)?;
                Ok(Param::Account(Some(key)))
            }
        )
    ]
}

fn to_str<E: std::fmt::Display>(e: E) -> String {
    format!("{}", e)
}
