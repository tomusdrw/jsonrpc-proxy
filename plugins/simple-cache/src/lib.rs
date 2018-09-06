//! A simplistic RPC cache.
//!
//! Caches the result of calling the RPC method and clears it
//! depending on the cache eviction policy.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate fnv;
extern crate jsonrpc_core as rpc;
extern crate parking_lot;
extern crate twox_hash;

use std::{
    collections::HashMap,
    sync::Arc,
    time,
};
use fnv::FnvHashMap;
use rpc::{
    futures::Future,
    futures::future::{self, Either},
};
use parking_lot::RwLock;

type Hash = String;

/// Describes what parameters should have separate caches.
#[derive(Debug)]
pub enum ParamsCache {
    /// Parameters for the method doesn't matter. Cache only by method name.
    IgnoreParams,
}

/// Cache eviction policy
#[derive(Debug)]
pub enum CacheEviction {
    /// Time-based caching. The cache entry is discarded after given amount of time.
    Time(time::Duration),
    // TODO [ToDr] notification (via subscription)
}

/// Method metadata
#[derive(Debug)]
enum MethodMeta {
    Deadline(time::Instant),
}

/// Represents a cacheable method.
///
/// Should know how to compute a hash that is used to compare requests.
/// TODO [ToDr] Support different eviction policies.
#[derive(Debug)]
pub struct Method {
    name: String,
    params: ParamsCache,
    eviction: CacheEviction,
}

impl Method {
    /// Create new method.
    pub fn new<T: Into<String>>(name: T, params: ParamsCache, eviction: CacheEviction) -> Self {
        Method {
            name: name.into(),
            params,
            eviction,
        }
    }

    /// Ignore parameters when caching.
    pub fn ignore_params<T: Into<String>>(name: T) -> Self {
        Self::new(name, ParamsCache::IgnoreParams, CacheEviction::Time(time::Duration::from_secs(5)))
    }

    /// Returns a hash of parameters of this method.
    fn hash(&self, _parameters: &Option<rpc::Params>) -> Hash {
        // TODO [ToDr] Should take parameters into account
        self.name.clone()
    }

    /// Generates metadata that should be stored in the cache together with the value.
    fn meta(&self) -> MethodMeta {
        match self.eviction {
            CacheEviction::Time(duration) => MethodMeta::Deadline(time::Instant::now() + duration),
        }
    }

    /// Determines if the cached result is still ok to use.
    fn is_fresh(&self, meta: &MethodMeta) -> bool {
        match *meta {
            MethodMeta::Deadline(deadline) => time::Instant::now() < deadline,
        }
    }
}

/// Simple single-level caching middleware.
///
/// Takes a list of cacheable methods as a parameter. Can construct multiple caches
/// for single method, based on the parameters.
#[derive(Debug)]
pub struct Middleware {
    cacheable: FnvHashMap<String, Method>,
    cached: Arc<RwLock<HashMap<
        Hash, 
        (Option<rpc::Output>, MethodMeta),
        ::twox_hash::RandomXxHashBuilder
    >>>,
}

impl Middleware {
    /// Creates new caching middleware given cacheable methods definitions.
    ///
    /// TODO [ToDr] Cache limits
    pub fn new(methods: Vec<Method>) -> Self {
        Middleware {
            cacheable: methods.into_iter().map(|x| (x.name.clone(), x)).collect(),
            cached: Default::default(),
        }
    }
}

impl<M: rpc::Metadata> rpc::Middleware<M> for Middleware {
    type Future = rpc::middleware::NoopFuture;
    type CallFuture = Either<
        rpc::middleware::NoopCallFuture,
        rpc::futures::future::FutureResult<Option<rpc::Output>, ()>,
    >;


    fn on_call<F, X>(&self, call: rpc::Call, meta: M, next: F) -> Either<Self::CallFuture, X> where
        F: FnOnce(rpc::Call, M) -> X + Send,
        X: Future<Item = Option<rpc::Output>, Error = ()> + Send + 'static, 
    {
        enum Action {
            Next,
            NextAndCache(Hash, MethodMeta),
            Return(Option<rpc::Output>),
        }

        let action = match call {
            rpc::Call::MethodCall(rpc::MethodCall { ref method, ref params, .. }) => {
                if let Some(method) = self.cacheable.get(method) {
                    let hash = method.hash(params);
                    if let Some((result, meta)) = self.cached.read().get(&hash) {
                        if method.is_fresh(meta) {
                            Action::Return(result.clone())
                        } else {
                            Action::NextAndCache(hash, method.meta())
                        }
                    } else {
                        Action::NextAndCache(hash, method.meta())
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
            // TODO [ToDr] Prevent multiple requests being made.
            Action::NextAndCache(hash, method_meta) => {
                let cached = self.cached.clone();
                Either::A(Either::A(Box::new(
                    next(call, meta)
                        .map(move |result| {
                            cached.write().insert(hash, (
                                result.clone(),
                                method_meta
                            ));
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
