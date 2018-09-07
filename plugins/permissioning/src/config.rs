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
