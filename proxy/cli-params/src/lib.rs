#![warn(missing_docs)]
#![warn(unused_extern_crates)]

/// Parses parameter value
pub trait Parser {
    type Executor;

    fn parse(&self, value: String) -> Result<Self::Executor, String>;
}

impl<F, X> Parser for F where
    F: Fn(String) -> Result<X, String>,
{
    type Executor = X;

    fn parse(&self, value: String) -> Result<Self::Executor, String> {
        (*self)(value)
    }
}

/// Describes a CLI parameter that should be present in the help.
pub struct Param<Exec> {
    pub category: String,
    pub name: String,
    pub description: String,
    pub default_value: String,
    pub parser: Box<Parser<Executor=Exec>>,
}

impl<X> Param<X> {
    pub fn parse(&self, value: Option<String>) -> Result<X, String> {
        let default_value = self.default_value.clone();
        let value = value.unwrap_or_else(|| default_value);

        self.parser.parse(value)
    }
}

// TODO [ToDr] ParamsBuilder to have nicer API
