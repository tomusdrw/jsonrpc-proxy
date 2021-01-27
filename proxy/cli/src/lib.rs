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
//! Builds clap CLI from multiple plugins.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate clap;
extern crate cli_params as params;

/// Adds plugin parameters to the CLI application.
pub fn configure_app<'a, 'b, Exec>(
    mut app: clap::App<'a, 'b>,
    params: &'a [params::Param<Exec>],
) -> clap::App<'a, 'b> {
    for p in params {
        app = app.arg(
            clap::Arg::with_name(&p.name)
                .long(&p.name)
                .takes_value(true)
                .help(&p.description)
                .default_value(&p.default_value),
        )
    }
    app
}

/// Extract parameters from CLI matches and turn them into parameters executors, which can be used
/// to configure particular transport or plugin.
pub fn parse_matches<Exec>(
    matches: &clap::ArgMatches,
    params: &[params::Param<Exec>],
) -> Result<Vec<Exec>, String> {
    params
        .iter()
        .map(|p| {
            let val = matches.value_of(&p.name);
            p.parse(val.map(str::to_owned))
        })
        .collect()
}
