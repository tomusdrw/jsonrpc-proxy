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
//! Upstream configuration parameters.

use std::{io, fs};
use cli_params;
use serde_json;
use Subscription;

/// Configuration options of an upstream
#[derive(Clone, Debug)]
pub enum Param {
    /// PublishSubscribe methods
    PubSubMethods(Vec<Subscription>),
}

/// Returns all configuration parameters for WS upstream.
pub fn params() -> Vec<cli_params::Param<Param>> {
    vec![
        cli_params::Param::new(
            "Upstream configuration",
            "upstream-config",
            "Configuration of the upstream. Should contain a list of supported pub-sub methods.",
            "-",
            move |path: String| {
                if &path == "-" {
                    return Ok(Param::PubSubMethods(Default::default()))
                }

                let file = fs::File::open(&path).map_err(|e| format!("Can't open upstream config file at {}: {:?}", path, e))?;
                let buf_file = io::BufReader::new(file);
                let config: Upstream = serde_json::from_reader(buf_file).map_err(|e| format!("Invalid JSON at {}: {:?}", path, e))?;
                Ok(Param::PubSubMethods(config.pubsub_methods))
            },
        )
    ]
}

/// Adds pubsub methods definitions to the existing parameter.
pub fn add_subscriptions(params: &mut [Param], methods: Vec<Subscription>) {
    for p in params {
        match p {
            Param::PubSubMethods(ref mut m) => {
                m.extend(methods.clone());
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
struct Upstream {
    pubsub_methods: Vec<Subscription>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_deserialize_example_configuration() {
        let _m: Upstream = serde_json::from_slice(include_bytes!("../../../examples/upstream.json")).unwrap();
    }
}
