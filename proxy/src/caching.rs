use std::{
    collections::HashMap,
    sync::Arc
};
use fnv::FnvHashMap;
use rpc::{
    self,
    futures::Future,
    futures::future::{self, Either},
};
use parking_lot::RwLock;

use super::Metadata;

type Hash = String;

/// Describes what parameters should have separate caches.
#[derive(Debug)]
pub enum ParamsCache {
    /// Parameters for the method doesn't matter. Cache only by method name.
    IgnoreParams
}

/// Represents a cacheable method.
///
/// Should know how to compute a hash that is used to compare requests.
/// TODO [ToDr] Support different eviction policies.
#[derive(Debug)]
pub struct Method {
    name: String,
    params: ParamsCache,
}

impl Method {
    /// Create new method.
    pub fn new<T: Into<String>>(name: T, params: ParamsCache) -> Self {
        Method {
            name: name.into(),
            params,
        }
    }

    /// Ignore parameters when caching.
    pub fn ignore_params<T: Into<String>>(name: T) -> Self {
        Self::new(name, ParamsCache::IgnoreParams)
    }

    /// Returns a hash of parameters of this method.
    pub fn hash(&self, _parameters: &Option<rpc::Params>) -> Hash {
        // TODO [ToDr] Should take parameters into account
        self.name.clone()
    }
}

/// Simple single-level caching middleware.
///
/// Takes a list of cacheable methods as a parameter. Can construct multiple caches
/// for single method, based on the parameters.
#[derive(Debug)]
pub struct Middleware {
    cacheable: FnvHashMap<String, Method>,
    cached: Arc<RwLock<HashMap<Hash, Option<rpc::Output>, ::twox_hash::RandomXxHashBuilder>>>,
}

impl Middleware {
    /// Creates new caching middleware given cacheable methods definitions.
    pub fn new(methods: Vec<Method>) -> Self {
        Middleware {
            cacheable: methods.into_iter().map(|x| (x.name.clone(), x)).collect(),
            cached: Default::default(),
        }
    }
}

impl rpc::Middleware<Metadata> for Middleware {
    type Future = rpc::middleware::NoopFuture;
    type CallFuture = Either<
        rpc::middleware::NoopCallFuture,
        rpc::futures::future::FutureResult<Option<rpc::Output>, ()>,
    >;


    fn on_call<F, X>(&self, call: rpc::Call, meta: Metadata, next: F) -> Either<Self::CallFuture, X> where
        F: FnOnce(rpc::Call, Metadata) -> X + Send,
        X: Future<Item = Option<rpc::Output>, Error = ()> + Send + 'static, 
    {
        enum Action {
            Next,
            NextAndCache(Hash),
            Return(Option<rpc::Output>),
        }

        let action = match call {
            rpc::Call::MethodCall(rpc::MethodCall { ref method, ref params, .. }) => {
                if let Some(method) = self.cacheable.get(method) {
                    let hash = method.hash(params);
                    if let Some(result) = self.cached.read().get(&hash) {
                        Action::Return(result.clone())
                    } else {
                        Action::NextAndCache(hash)
                    }
                } else {
                    Action::Next
                }
            },
            _ => Action::Next,
        };

        match action {
            // Fallback
            Action::Next => Either::B(next(call, meta)),
            Action::NextAndCache(hash) => {
                let cached = self.cached.clone();
                Either::A(Either::A(Box::new(
                    next(call, meta)
                        .map(move |result| {
                            cached.write().insert(hash, result.clone());
                            result
                        })
                )))
            },
            Action::Return(result) => {
                Either::A(Either::B(future::done(Ok(result))))
            }
        }

    }
}

#[cfg(test)]
mod tests {

}
