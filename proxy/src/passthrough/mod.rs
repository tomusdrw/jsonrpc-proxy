use std::{
    collections::HashMap,
    sync::Arc,
};

use pubsub;
use rpc::{
    self,
    futures::Future,
    futures::future::Either,
};

use super::Metadata;

mod helpers;
pub mod ws;

#[derive(Debug, Clone)]
pub struct Subscription {
    pub subscribe: String,
    pub unsubscribe: String,
    pub name: String,
}

pub trait Transport: Send + Sync + 'static {
    type Error;
    type Future: Future<Item = Option<rpc::Output>, Error = Self::Error> + Send + 'static;

    fn subscribe(
        &self,
        call: rpc::Call,
        sink: Option<Arc<pubsub::Session>>,
        subscription: Subscription,
    ) -> Self::Future;

    fn unsubscribe(
        &self,
        call: rpc::Call,
        subscription: Subscription,
    ) -> Self::Future;

    fn send(&self, call: rpc::Call) -> Self::Future;
}

#[derive(Debug)]
pub struct Middleware<T> {
    transport: T,
    subscribe_methods: HashMap<String, Subscription>,
    unsubscribe_methods: HashMap<String, Subscription>,
}

impl<T> Middleware<T> {
    pub fn new(
        transport: T,
        pubsub_methods: Vec<Subscription>,
    ) -> Self {
        Self {
            transport,
            subscribe_methods: pubsub_methods.iter().map(|s| (s.subscribe.clone(), s.clone())).collect(),
            unsubscribe_methods: pubsub_methods.into_iter().map(|s| (s.unsubscribe.clone(), s)).collect(),
        }
    }
}

impl<T: Transport + 'static> rpc::Middleware<Metadata> for Middleware<T> {
    type Future = rpc::middleware::NoopFuture;
    type CallFuture = rpc::middleware::NoopCallFuture;

    fn on_call<F, X>(&self, request: rpc::Call, meta: Metadata, next: F) -> Either<Self::CallFuture, X> where
        F: FnOnce(rpc::Call, Metadata) -> X + Send,
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
                self.transport.subscribe(request, meta, subscription).map_err(|e| ())
            ))
        }

        if let Some(subscription) = unsubscribe {
            return Either::A(Box::new(
                self.transport.unsubscribe(request, subscription).map_err(|e| ())
            ))
        }

        Either::A(Box::new(
            self.transport.send(request).map_err(|e| ())
        ))
    }
}
