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

#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
struct Upstream {
    pubsub_methods: Vec<Subscription>,
}
