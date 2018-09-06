//! Builds clap CLI from multiple plugins.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate clap;
extern crate cli_params as params;

/// Adds plugin parameters to the CLI application.
pub fn configure_app<'a, 'b, Exec>(mut app: clap::App<'a, 'b>, params: &'a [params::Param<Exec>]) -> clap::App<'a, 'b> {
    for p in params {
        // TODO [ToDr] Use prefix
        app = app.arg(clap::Arg::with_name(&p.arg)
            .long(&p.name)
            .takes_value(true)
            .help(&p.description)
            .default_value(&p.default_value)
        )
    }
    app
}

/// Extract parameters from CLI matches and turn them into parameters executors, which can be used
/// to configure particular transport or plugin.
pub fn parse_matches<Exec>(matches: &clap::ArgMatches, params: &[params::Param<Exec>]) -> Result<Vec<Exec>, String> {
    params
        .iter()
        .map(|p| {
            let val = matches.value_of(&p.arg);
            p.parse(val.map(str::to_owned))
        })
        .collect()
}
