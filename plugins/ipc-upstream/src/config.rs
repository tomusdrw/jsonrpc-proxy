//! IPC upstream configuration parameters.

use cli_params;

/// Configuration options of the IPC upstream
pub enum Param {
    /// Upstream URL
    Path(String),
}

/// Returns all configuration parameters for IPC upstream.
pub fn params() -> Vec<cli_params::Param<Param>> {
    vec![
        cli_params::Param::new(
            "IPC upstream",
            "upstream-ipc",
            "Path to the IPC socket we should connect to.",
            "/var/tmp/parity.ipc",
            move |val: String| {
                Ok(Param::Path(val))
            },
        )
    ]
}
