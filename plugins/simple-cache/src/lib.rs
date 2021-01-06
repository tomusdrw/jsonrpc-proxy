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
//! A simplistic RPC cache.
//!
//! Caches the result of calling the RPC method and clears it
//! depending on the cache eviction policy.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate cli_params;
extern crate fnv;
extern crate jsonrpc_core as rpc;
extern crate parking_lot;
extern crate serde_json;
extern crate twox_hash;

#[macro_use]
extern crate serde_derive;

use std::{
    io,
    hash::{Hash as HashTrait, Hasher},
    sync::Arc,
    time,
};
use fnv::FnvHashMap;
use rpc::{
    futures::Future,
    futures::future::{self, Either},
};
use parking_lot::RwLock;

type Hash = u64;

pub mod config;

/// Cache eviction policy
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Clone, Debug, Deserialize)]
pub struct Method {
    name: String,
    eviction: CacheEviction,
}

impl Method {
    /// Create new method.
    pub fn new<T: Into<String>>(name: T, eviction: CacheEviction) -> Self {
        Method {
            name: name.into(),
            eviction,
        }
    }

    /// Returns a hash of parameters of this method.
    fn hash(&self, parameters: &rpc::Params) -> Hash {
        let mut hasher = twox_hash::XxHash::default();
        self.name.hash(&mut hasher);
        serde_json::to_writer(HashWriter(&mut hasher), parameters).expect("HashWriter never fails.");
        hasher.finish()
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
    enabled: bool,
    cacheable: FnvHashMap<String, Method>,
    cached: Arc<RwLock<FnvHashMap<
        Hash, 
        (Option<rpc::Output>, MethodMeta),
    >>>,
}

impl Middleware {
    /// Creates new caching middleware given cacheable methods definitions.
    ///
    /// TODO [ToDr] Cache limits
    pub fn new(params: &[config::Param]) -> Self {
        let mut cache = config::Cache::default();
        for p in params {
            match p {
                config::Param::Config(ref m) => cache = m.clone(),
            }
        }

        Middleware {
            enabled: cache.enabled,
            cacheable: cache.methods.into_iter().map(|x| (x.name.clone(), x)).collect(),
            cached: Default::default(),
        }
    }
}

impl<M: rpc::Metadata> rpc::Middleware<M> for Middleware {
    type Future = rpc::middleware::NoopFuture;
    type CallFuture = Either<
        rpc::middleware::NoopCallFuture,
        rpc::futures::future::Ready<Option<rpc::Output>>,
    >;


    fn on_call<F, X>(&self, call: rpc::Call, meta: M, next: F) -> Either<Self::CallFuture, X> where
        F: FnOnce(rpc::Call, M) -> X + Send,
        X: Future<Output = Option<rpc::Output>> + Send + 'static, 
    {
        use rpc::futures::FutureExt;

        if !self.enabled {
            return Either::Right(next(call, meta));
        }

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
            Action::Next => Either::Right(next(call, meta)),
            // TODO [ToDr] Prevent multiple requests being made.
            Action::NextAndCache(hash, method_meta) => {
                let cached = self.cached.clone();
                Either::Left(Either::Left(Box::pin(
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
                Either::Left(Either::Right(future::ready(result)))
            }
        }

    }
}

struct HashWriter<'a, W: 'a>(&'a mut W);

impl<'a, W: 'a + Hasher> io::Write for HashWriter<'a, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        buf.hash(self.0);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{atomic, Arc};
    use rpc::Middleware as MiddlewareTrait;
    use super::*;

    trait FutExt: std::future::Future {
        fn wait(self) -> Self::Output;
    }

    impl<F> FutExt for F where
        F: std::future::Future,
    {
        fn wait(self) -> Self::Output {
            rpc::futures::executor::block_on(self)
        }
    }

    fn callback() -> (
        impl Fn(rpc::Call, ()) -> rpc::futures::future::Ready<Option<rpc::Output>>,
        Arc<atomic::AtomicUsize>,
    ) {
        let called = Arc::new(atomic::AtomicUsize::new(0));
        let called2 = called.clone();
        let next = move |_, _| {
            called2.fetch_add(1, atomic::Ordering::SeqCst);
            rpc::futures::future::ready(None)
        };

        (next, called)
    }

    fn method_call(name: &str, param: &str) -> rpc::Call {
        rpc::Call::MethodCall(rpc::MethodCall {
            id: rpc::Id::Num(1),
            jsonrpc: Some(rpc::Version::V2),
            method: name.into(),
            params: rpc::Params::Array(vec![param.into()]),
        })
    }

    fn middleware(config: config::Cache) -> Middleware {
        Middleware::new(&[
            config::Param::Config(config)
        ])
    }

    #[test]
    fn should_forward_if_cache_disabled() {
        // given
        let middleware = middleware(config::Cache {
            enabled: false,
            methods: vec![
                Method::new("eth_getBlock", CacheEviction::Time(time::Duration::from_secs(1))),
            ],
        });
        let (next, called) = callback();

        // when
        let res1 = middleware.on_call(method_call("eth_getBlock", "xyz"), (), &next).wait();
        let res2 = middleware.on_call(method_call("eth_getBlock", "xyz"), (), &next).wait();

        // then
        assert_eq!(called.load(atomic::Ordering::SeqCst), 2);
        assert_eq!(res1, None);
        assert_eq!(res2, None);
    }

    #[test]
    fn should_return_cached_result() {
        // given
        let middleware = middleware(config::Cache {
            enabled: true,
            methods: vec![
                Method::new("eth_getBlock", CacheEviction::Time(time::Duration::from_secs(1))),
            ],
        });
        let (next, called) = callback();

        // when
        let res1 = middleware.on_call(method_call("eth_getBlock", "xyz"), (), &next).wait();
        let res2 = middleware.on_call(method_call("eth_getBlock", "xyz"), (), &next).wait();

        // then
        assert_eq!(called.load(atomic::Ordering::SeqCst), 1);
        assert_eq!(res1, None);
        assert_eq!(res2, None);
    }

    #[test]
    fn should_not_cache_when_params_different() {
        // given
        let middleware = middleware(config::Cache {
            enabled: true,
            methods: vec![
                Method::new("eth_getBlock", CacheEviction::Time(time::Duration::from_secs(1))),
            ],
        });
        let (next, called) = callback();

        // when
        let res1 = middleware.on_call(method_call("eth_getBlock", "xyz1"), (), &next).wait();
        let res2 = middleware.on_call(method_call("eth_getBlock", "xyz2"), (), &next).wait();

        // then
        assert_eq!(called.load(atomic::Ordering::SeqCst), 2);
        assert_eq!(res1, None);
        assert_eq!(res2, None);
    }

    #[test]
    fn should_invalidate_cache_after_specified_time() {
        // given
        let middleware = middleware(config::Cache {
            enabled: true,
            methods: vec![
                Method::new("eth_getBlock", CacheEviction::Time(time::Duration::from_millis(1))),
            ],
        });
        let (next, called) = callback();

        // when
        let res1 = middleware.on_call(method_call("eth_getBlock", "xyz"), (), &next).wait();
        let res2 = middleware.on_call(method_call("eth_getBlock", "xyz"), (), &next).wait();
        ::std::thread::sleep(time::Duration::from_millis(2));
        let res3 = middleware.on_call(method_call("eth_getBlock", "xyz"), (), &next).wait();

        // then
        assert_eq!(called.load(atomic::Ordering::SeqCst), 2);
        assert_eq!(res1, None);
        assert_eq!(res2, None);
        assert_eq!(res3, None);
    }

    // TODO [ToDr] Implement me
    #[ignore]
    #[test]
    fn should_never_send_request_twice() {
        // given
        let middleware = middleware(config::Cache {
            enabled: true,
            methods: vec![
                Method::new("eth_getBlock", CacheEviction::Time(time::Duration::from_secs(1))),
            ],
        });
        let (next, called) = callback();

        // when
        let res1 = middleware.on_call(method_call("eth_getBlock", "xyz"), (), &next);
        let res2 = middleware.on_call(method_call("eth_getBlock", "xyz"), (), &next);

        // then
        assert_eq!(called.load(atomic::Ordering::SeqCst), 1);
        assert_eq!(res1.wait(), None);
        assert_eq!(res2.wait(), None);
    }

}
