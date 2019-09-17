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
