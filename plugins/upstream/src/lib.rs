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

use std::{collections::HashMap, sync::Arc};

use rpc::futures::{future::Either, Future};

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
    type Future: Future<Output = Result<Option<rpc::Output>, Self::Error>> + Send + Unpin + 'static;

    /// Send subscribe call upstream.
    fn subscribe(
        &self,
        call: rpc::Call,
        sink: Option<Arc<pubsub::Session>>,
        subscription: Subscription,
    ) -> Self::Future;

    /// Send unsubscribe call upstream.
    fn unsubscribe(&self, call: rpc::Call, subscription: Subscription) -> Self::Future;

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
    pub fn new(transport: T, params: &[config::Param]) -> Self {
        let mut pubsub_methods = vec![];
        for p in params {
            match p {
                config::Param::PubSubMethods(ref m) => pubsub_methods.extend(m.clone()),
            }
        }

        Self {
            transport,
            subscribe_methods: pubsub_methods
                .iter()
                .map(|s| (s.subscribe.clone(), s.clone()))
                .collect(),
            unsubscribe_methods: pubsub_methods.into_iter().map(|s| (s.unsubscribe.clone(), s)).collect(),
        }
    }
}

impl<T, M> rpc::Middleware<M> for Middleware<T>
where
    T: Transport + 'static,
    M: rpc::Metadata + Into<Option<Arc<pubsub::Session>>>,
{
    type Future = rpc::middleware::NoopFuture;
    type CallFuture = rpc::middleware::NoopCallFuture;

    fn on_call<F, X>(&self, request: rpc::Call, meta: M, _next: F) -> Either<Self::CallFuture, X>
    where
        F: FnOnce(rpc::Call, M) -> X + Send,
        X: Future<Output = Option<rpc::Output>> + Send + 'static,
    {
        use rpc::futures::{FutureExt, TryFutureExt};

        let (subscribe, unsubscribe) = {
            let method = helpers::get_method_name(&request);
            if let Some(method) = method {
                match self.subscribe_methods.get(method).cloned() {
                    Some(subscription) => (Some(subscription), None),
                    None => (None, self.unsubscribe_methods.get(method).cloned()),
                }
            } else {
                (None, None)
            }
        };

        if let Some(subscription) = subscribe {
            return Either::Left(Box::pin(
                self.transport
                    .subscribe(request, meta.into(), subscription)
                    .map_err(|e| warn!("Failed to subscribe: {:?}", e))
                    .map(|v| v.unwrap_or(None)),
            ));
        }

        if let Some(subscription) = unsubscribe {
            return Either::Left(Box::pin(
                self.transport
                    .unsubscribe(request, subscription)
                    .map_err(|e| warn!("Failed to unsubscribe: {:?}", e))
                    .map(|v| v.unwrap_or(None)),
            ));
        }

        Either::Left(Box::pin(
            self.transport
                .send(request)
                .map_err(|e| warn!("Failed to send: {:?}", e))
                .map(|v| v.unwrap_or(None)),
        ))
    }
}
