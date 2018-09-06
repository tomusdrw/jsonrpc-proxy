//! CLI configuration for simple cache.

use std::{fs, io};
use cli_params;
use serde_json;
use Method;

/// A configuration option to apply.
pub enum Param {
    /// Methods that should be cached.
    CachedMethods(Vec<Method>),
}

/// Returns a list of supported configuration parameters.
pub fn params() -> Vec<cli_params::Param<Param>> {
    vec![
        cli_params::Param::new(
            "Simple Cache",
            "cached-methods-path",
            "A path to a JSON file containing a list of methods that should be cached. See examples for the file schema.",
            "-",
            |path: String| {
                if &path == "-" {
                    return Ok(Param::CachedMethods(vec![]))
                }

                let file = fs::File::open(&path).map_err(|e| format!("Can't open cache file at {}: {:?}", path, e))?;
                let buf_file = io::BufReader::new(file);
                let methods: CacheMethods = serde_json::from_reader(buf_file).map_err(|e| format!("Invalid JSON at {}: {:?}", path, e))?;
                Ok(Param::CachedMethods(methods.0))
            }
        )
    ]
}

#[derive(Deserialize)]
struct CacheMethods(Vec<Method>);


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_deserialize_example() {
        let _m: CacheMethods = serde_json::from_slice(include_bytes!("../../../examples/cache.json")).unwrap();
    }
}
