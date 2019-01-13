//! Upstream base crate.
//!
//! Used by specific implementations and contains abstract logic that can be re-used between them.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate cli_params;
extern crate jsonrpc_core as rpc;
extern crate jsonrpc_pubsub as pubsub;
extern crate parking_lot;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

use std::{
    collections::HashMap,
    sync::Arc,
};

use rpc::{
    futures::Future,
    futures::future::Either,
};

pub mod config;
pub mod helpers;
pub mod shared;

/// Represents a Pub-Sub method description.
#[derive(Debug, Clone, Deserialize)]
pub struct Subscription {
    /// Subscribe method name.
    pub subscribe: String,
    /// Unsubscribe method name.
    pub unsubscribe: String,
    /// A method for notifications for that subscription.
    pub name: String,
}

/// Passthrough transport.
///
/// This is an upstream transport (can do load balancing, failover or parallel requests)
pub trait Transport: Send + Sync + 'static {
    /// Error type of the transport.
    type Error: ::std::fmt::Debug;
    /// Future returned by the transport.
    type Future: Future<Item = Option<rpc::Output>, Error = Self::Error> + Send + 'static;

    /// Send subscribe call upstream.
    fn subscribe(
        &self,
        call: rpc::Call,
        sink: Option<Arc<pubsub::Session>>,
        subscription: Subscription,
    ) -> Self::Future;

    /// Send unsubscribe call upstream.
    fn unsubscribe(
        &self,
        call: rpc::Call,
        subscription: Subscription,
    ) -> Self::Future;

    /// Send a regular call upstream.
    fn send(&self, call: rpc::Call) -> Self::Future;
}

/// Pass-through middleware
///
/// Delegates the calls to the upstream `Transport` - should be used as the last middleware,
/// since it never calls `next`.
#[derive(Debug)]
pub struct Middleware<T> {
    transport: T,
    subscribe_methods: HashMap<String, Subscription>,
    unsubscribe_methods: HashMap<String, Subscription>,
}

impl<T> Middleware<T> {
    /// Create new passthrough middleware with given upstream and the list of pubsub methods.
    pub fn new(
        transport: T,
        params: &[config::Param],
    ) -> Self {
        let mut pubsub_methods = vec![];
        for p in params {
            match p {
                config::Param::PubSubMethods(ref m) => pubsub_methods.extend(m.clone()),
                _ => {}
            }
        }

        Self {
            transport,
            subscribe_methods: pubsub_methods.iter().map(|s| (s.subscribe.clone(), s.clone())).collect(),
            unsubscribe_methods: pubsub_methods.into_iter().map(|s| (s.unsubscribe.clone(), s)).collect(),
        }
    }
}

impl<T, M> rpc::Middleware<M> for Middleware<T> where
    T: Transport + 'static,
    M: rpc::Metadata + Into<Option<Arc<pubsub::Session>>>,
{
    type Future = rpc::middleware::NoopFuture;
    type CallFuture = rpc::middleware::NoopCallFuture;

    fn on_call<F, X>(&self, request: rpc::Call, meta: M, _next: F) -> Either<Self::CallFuture, X> where
        F: FnOnce(rpc::Call, M) -> X + Send,
        X: Future<Item = Option<rpc::Output>, Error = ()> + Send + 'static, 
    {
        let (subscribe, unsubscribe) = {
            let method = helpers::get_method_name(&request);
            if let Some(method) = method {
                match self.subscribe_methods.get(method).cloned() {
                    Some(subscription) => (Some(subscription), None),
                    None => (None, self.unsubscribe_methods.get(method).cloned())
                }
            } else {
                (None, None)
            }
        };

        if let Some(subscription) = subscribe {
            return Either::A(Box::new(
                self.transport.subscribe(request, meta.into(), subscription).map_err(|e| {
                    warn!("Failed to subscribe: {:?}", e);
                })
            ))
        }

        if let Some(subscription) = unsubscribe {
            return Either::A(Box::new(
                self.transport.unsubscribe(request, subscription).map_err(|e| {
                    warn!("Failed to unsubscribe: {:?}", e);
                })
            ))
        }

        Either::A(Box::new(
            self.transport.send(request).map_err(|e| {
                warn!("Failed to send: {:?}", e);
            })
        ))
    }
}
