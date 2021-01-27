// Copyright (c) 2018-2020 jsonrpc-proxy contributors.
//
// This file is part of jsonrpc-proxy
// (see https://github.com/tomusdrw/jsonrpc-proxy).
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.
//! A base type for parameter definition.
//!
//! Intendend to be used by plugins to expose configurable parameters.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

/// Parses parameter value
pub trait Parser {
    /// A type that can apply parsed parameter.
    type Executor;

    /// Parse parameter and return the Executor.
    fn parse(&self, value: String) -> Result<Self::Executor, String>;
}

impl<F, X> Parser for F
where
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
    pub parser: Box<dyn Parser<Executor = Exec>>,
}

impl<X> Param<X> {
    /// Create new parameter definition.
    pub fn new<A, B, C, D, E>(category: A, name: B, description: C, default_value: D, parser: E) -> Self
    where
        A: Into<String>,
        B: Into<String>,
        C: Into<String>,
        D: Into<String>,
        E: Parser<Executor = X> + 'static,
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
