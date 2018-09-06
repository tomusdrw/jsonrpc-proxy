//! WebSocket upstream configuration parameters.

use cli_params::Param;

/// Configuration options of the WS upstream
pub enum Configuration {
    /// Upstream URL
    Url(::websocket::url::Url),
}

/// Returns all configuration parameters for WS upstream.
pub fn all() -> Vec<Param<Configuration>> {
    vec![
        Param {
            category: "WebSockets upstream".into(),
            name: "upstream-ws".into(),
            description: "Address of the parent WebSockets RPC server that we should connect to.".into(),
            default_value: "127.0.0.1:9944".into(),
            parser: Box::new(move |val: String| {
                let url = val.parse().map_err(|e| format!("Invalid upstream address: {:?}", e))?;
                Ok(Configuration::Url(url))
            }),
        }
    ]
}
