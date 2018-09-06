//! WebSocket upstream configuration parameters.

use cli_params;

/// Configuration options of the WS upstream
pub enum Param {
    /// Upstream URL
    Url(::websocket::url::Url),
}

/// Returns all configuration parameters for WS upstream.
pub fn params() -> Vec<cli_params::Param<Param>> {
    vec![
        cli_params::Param::new(
            "WebSockets upstream",
            "upstream-ws",
            "Address of the parent WebSockets RPC server that we should connect to.",
            "ws://127.0.0.1:9944",
            move |val: String| {
                let url = val.parse().map_err(|e| format!("Invalid upstream address: {:?}", e))?;
                Ok(Param::Url(url))
            },
        )
    ]
}
