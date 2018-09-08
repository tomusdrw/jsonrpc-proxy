//! CLI configuration for simple cache.

use std::{fs, io};
use cli_params;
use serde_json;
use Method;

/// A configuration option to apply.
pub enum Param {
    /// Methods that should be cached.
    CachedMethods(Cache),
}

/// Returns a list of supported configuration parameters.
pub fn params() -> Vec<cli_params::Param<Param>> {
    vec![
        cli_params::Param::new(
            "Simple Cache",
            "simple-cache-config",
            "A path to a JSON file containing a list of methods that should be cached. See examples for the file schema.",
            "-",
            |path: String| {
                if &path == "-" {
                    return Ok(Param::CachedMethods(Default::default()))
                }

                let file = fs::File::open(&path).map_err(|e| format!("Can't open cache file at {}: {:?}", path, e))?;
                let buf_file = io::BufReader::new(file);
                let methods: Cache = serde_json::from_reader(buf_file).map_err(|e| format!("Invalid JSON at {}: {:?}", path, e))?;
                Ok(Param::CachedMethods(methods))
            }
        )
    ]
}

/// Add methods given as the first parameter to the config in one of the params.
pub fn add_methods(params: &mut [Param], methods: Vec<Method>) {
    for p in params {
        match p {
            Param::CachedMethods(ref mut config) => {
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
    pub methods: Vec<Method>
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
