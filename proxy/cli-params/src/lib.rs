#![warn(missing_docs)]
#![warn(unused_extern_crates)]

/// Parses parameter value
pub trait Parser {
    /// A type that can apply parsed parameter.
    type Executor;

    /// Parse parameter and return the Executor.
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
    /// Parameters category
    pub category: String,
    /// Parameter name
    pub name: String,
    /// Parameter description
    pub description: String,
    /// Parameter default value
    pub default_value: String,
    /// Parameter parser
    pub parser: Box<Parser<Executor=Exec>>,
}

impl<X> Param<X> {
    /// Create new parameter definition.
    pub fn new<A, B, C, D, E>(
        category: A, 
        name: B,
        description: C,
        default_value: D,
        parser: E,
    ) -> Self where
        A: Into<String>,
        B: Into<String>,
        C: Into<String>,
        D: Into<String>,
        E: Parser<Executor=X> + 'static,
    {
        Param {
            category: category.into(),
            name: name.into(),
            description: description.into(),
            default_value: default_value.into(),
            parser: Box::new(parser),
        }
    }

    /// Parse given value and return `Executor` for given param.
    pub fn parse(&self, value: Option<String>) -> Result<X, String> {
        let default_value = self.default_value.clone();
        let value = value.unwrap_or_else(|| default_value);

        self.parser.parse(value)
    }
}

// TODO [ToDr] ParamsBuilder to have nicer API
