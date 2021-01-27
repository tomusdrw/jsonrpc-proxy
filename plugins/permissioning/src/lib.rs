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
//! A simple permissioning system.
//!
//! Allows you to turn off particular methods.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate cli_params;
extern crate fnv;
extern crate jsonrpc_core as rpc;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

use fnv::FnvHashMap;
use rpc::futures::{future::Either, Future};

pub mod config;

/// Describes method access.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Access {
    /// Allow all access to that method
    Allow,
    /// Deny any access to that method
    Deny,
    // TODO [ToDr] Add other policies like:
    // 1. Require authorization header (fixed)
    // 2. Require HTTP basic credentials
    // 3. Allow only over specific transport
    // (All will require extending the metadata to contain this info)
}

/// Represents a managed method.
///
/// Should know how to compute a hash that is used to compare requests.
#[derive(Clone, Debug, Deserialize)]
pub struct Method {
    /// Method name
    pub name: String,
    /// Method access details
    pub policy: Access,
}

/// Represents permissioning configuration
#[derive(Clone, Debug, Deserialize)]
pub struct Permissioning {
    /// Default (base) policy
    pub policy: Access,
    /// Method overrides
    pub methods: Vec<Method>,
}

impl Default for Permissioning {
    fn default() -> Self {
        Permissioning {
            policy: Access::Allow,
            methods: Default::default(),
        }
    }
}

/// Simple static permissioning scheme
#[derive(Debug)]
pub struct Middleware {
    base: Access,
    permissioned: FnvHashMap<String, Method>,
}

impl Middleware {
    /// Creates new permissioning middleware
    pub fn new(params: &[config::Param]) -> Self {
        let mut config = Permissioning::default();
        for p in params {
            match p {
                config::Param::Config(ref m) => config = m.clone(),
            }
        }

        Middleware {
            base: config.policy,
            permissioned: config.methods.into_iter().map(|x| (x.name.clone(), x)).collect(),
        }
    }
}

impl<M: rpc::Metadata> rpc::Middleware<M> for Middleware {
    type Future = rpc::middleware::NoopFuture;
    type CallFuture = rpc::futures::future::Ready<Option<rpc::Output>>;

    fn on_call<F, X>(&self, call: rpc::Call, meta: M, next: F) -> Either<Self::CallFuture, X>
    where
        F: Fn(rpc::Call, M) -> X + Send,
        X: Future<Output = Option<rpc::Output>> + Send + 'static,
    {
        enum Action {
            Next,
            Reject,
        }

        let to_action = |access: &Access| match *access {
            Access::Allow => Action::Next,
            Access::Deny => Action::Reject,
        };

        let action = {
            match call {
                rpc::Call::MethodCall(rpc::MethodCall { ref method, .. }) => {
                    if let Some(m) = self.permissioned.get(method) {
                        to_action(&m.policy)
                    } else {
                        to_action(&self.base)
                    }
                }
                _ => to_action(&self.base),
            }
        };

        match action {
            Action::Next => Either::Right(next(call, meta)),
            Action::Reject => {
                let (version, id) = get_call_details(call);

                Either::Left(rpc::futures::future::ready(id.map(|id| {
                    rpc::Output::Failure(rpc::Failure {
                        jsonrpc: version,
                        error: rpc::Error {
                            code: rpc::ErrorCode::ServerError(-1),
                            message: "You are not allowed to call that method.".into(),
                            data: None,
                        },
                        id,
                    })
                })))
            }
        }
    }
}

fn get_call_details(call: rpc::Call) -> (Option<rpc::Version>, Option<rpc::Id>) {
    match call {
        rpc::Call::MethodCall(rpc::MethodCall { jsonrpc, id, .. }) => (jsonrpc, Some(id)),
        rpc::Call::Notification(rpc::Notification { jsonrpc, .. }) => (jsonrpc, None),
        rpc::Call::Invalid { id, .. } => (None, Some(id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rpc::Middleware as MiddlewareTrait;
    use std::sync::{atomic, Arc};

    trait FutExt: std::future::Future {
        fn wait(self) -> Self::Output;
    }

    impl<F> FutExt for F
    where
        F: std::future::Future,
    {
        fn wait(self) -> Self::Output {
            rpc::futures::executor::block_on(self)
        }
    }

    fn callback() -> (
        impl Fn(rpc::Call, ()) -> rpc::futures::future::Ready<Option<rpc::Output>>,
        Arc<atomic::AtomicBool>,
    ) {
        let called = Arc::new(atomic::AtomicBool::new(false));
        let called2 = called.clone();
        let next = move |_, _| {
            called2.store(true, atomic::Ordering::SeqCst);
            rpc::futures::future::ready(None)
        };

        (next, called)
    }

    fn method_call(name: &str) -> rpc::Call {
        rpc::Call::MethodCall(rpc::MethodCall {
            id: rpc::Id::Num(1),
            jsonrpc: Some(rpc::Version::V2),
            method: name.into(),
            params: rpc::Params::Array(vec![]),
        })
    }

    fn middleware(config: Permissioning) -> Middleware {
        Middleware::new(&[config::Param::Config(config)])
    }

    fn not_allowed() -> Option<rpc::Output> {
        Some(rpc::Output::Failure(rpc::Failure {
            id: rpc::Id::Num(1),
            error: rpc::Error {
                code: rpc::ErrorCode::ServerError(-1),
                message: "You are not allowed to call that method.".into(),
                data: None,
            },
            jsonrpc: Some(rpc::Version::V2),
        }))
    }

    #[test]
    fn should_allow_method_by_global_policy() {
        // given
        let middleware = middleware(Default::default());
        let (next, called) = callback();

        // when
        let result = middleware.on_call(method_call("eth_getBlock"), (), next);

        // then
        assert_eq!(called.load(atomic::Ordering::SeqCst), true);
        assert_eq!(result.wait(), None);
    }

    #[test]
    fn should_deny_blacklisted_method() {
        // given
        let middleware = middleware(Permissioning {
            policy: Access::Allow,
            methods: vec![Method {
                name: "eth_getBlock".into(),
                policy: Access::Deny,
            }],
        });
        let (next, called) = callback();

        // when
        let result = middleware.on_call(method_call("eth_getBlock"), (), next);

        // then
        assert_eq!(called.load(atomic::Ordering::SeqCst), false);
        assert_eq!(result.wait(), not_allowed());
    }

    #[test]
    fn should_deny_method_by_global_policy() {
        // given
        let middleware = middleware(Permissioning {
            policy: Access::Deny,
            methods: vec![],
        });
        let (next, called) = callback();

        // when
        let result = middleware.on_call(method_call("eth_getBlock"), (), next);

        // then
        assert_eq!(called.load(atomic::Ordering::SeqCst), false);
        assert_eq!(result.wait(), not_allowed());
    }

    #[test]
    fn should_allow_whitelisted_method() {
        // given
        let middleware = middleware(Permissioning {
            policy: Access::Deny,
            methods: vec![Method {
                name: "eth_getBlock".into(),
                policy: Access::Allow,
            }],
        });
        let (next, called) = callback();

        // when
        let result = middleware.on_call(method_call("eth_getBlock"), (), next);

        // then
        assert_eq!(called.load(atomic::Ordering::SeqCst), true);
        assert_eq!(result.wait(), None);
    }
}
