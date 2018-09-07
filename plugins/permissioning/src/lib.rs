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
use rpc::{
    futures::Future,
    futures::future::Either,
};

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
    type CallFuture = rpc::futures::future::FutureResult<Option<rpc::Output>, ()>;

    fn on_call<F, X>(&self, call: rpc::Call, meta: M, next: F) -> Either<Self::CallFuture, X> where
        F: FnOnce(rpc::Call, M) -> X + Send,
        X: Future<Item = Option<rpc::Output>, Error = ()> + Send + 'static, 
    {
        enum Action {
            Next,
            Reject
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
                },
                _ => to_action(&self.base),
            }
        };

        match action {
            Action::Next => {
                Either::B(next(call, meta))
            },
            Action::Reject => {
                let (version, id) = get_call_details(call);

                Either::A(rpc::futures::future::ok(id.map(|id| {
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
