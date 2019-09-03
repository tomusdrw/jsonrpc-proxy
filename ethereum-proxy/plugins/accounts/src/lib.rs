//! A simplistic RPC cache.
//!
//! Caches the result of calling the RPC method and clears it
//! depending on the cache eviction policy.

#![warn(missing_docs)]

use std::sync::Arc;
use jsonrpc_core::{
    self as rpc,
    futures::Future,
    futures::future::{self, Either},
};
use ethsign::{SecretKey, Protected, KeyFile};

pub mod config;

type Upstream = Box<
    dyn Fn(rpc::Call) -> Box<
        dyn Future<Item=Option<rpc::Output>, Error=()>
        + Send
    >
    + Send
    + Sync
>;

#[derive(Clone)]
pub struct Middleware {
    secret: Option<SecretKey>,
    upstream: Arc<Upstream>,
}

impl Middleware {
    /// Creates a new signing middleware.
    ///
    /// Intercepts calls to `eth_sendTransaction` and replaces them
    /// with `eth_sendRawTransaction`.
    pub fn new(upstream: Arc<Upstream>, params: &[config::Param]) -> Self {
        let mut key = None;
        let mut pass: Protected = "".into();

        for p in params {
            match p {
                config::Param::Account(k) => key = k.clone(),
                config::Param::Pass(p) => pass = p.clone(),
            }
        }

        let secret = key.map(|key: KeyFile| {
            // TODO [ToDr] Panicking here is crap.
            key.to_secret_key(&pass).unwrap()
        });
     
        Self {
            secret,
            upstream,
        }
    }
}

impl<M: rpc::Metadata> rpc::Middleware<M> for Middleware {
    type Future = rpc::middleware::NoopFuture;
    type CallFuture = Either<
        rpc::middleware::NoopCallFuture,
        rpc::futures::future::FutureResult<Option<rpc::Output>, ()>,
    >;

    fn on_call<F, X>(&self, mut call: rpc::Call, meta: M, next: F) -> Either<Self::CallFuture, X> where
        F: FnOnce(rpc::Call, M) -> X + Send,
        X: Future<Item = Option<rpc::Output>, Error = ()> + Send + 'static, 
    {
        let secret = match self.secret.as_ref() {
            Some(secret) => secret,
            None => return Either::B(next(call, meta)),
        };

        let (jsonrpc, id) = match &mut call {
            &mut rpc::Call::MethodCall(rpc::MethodCall { ref mut method, ref jsonrpc, ref id, .. })
                if method == "eth_sendTransaction" => {
                *method = "parity_composeTransaction".into();
                (jsonrpc.clone(), id.clone())
            },
            _ => return Either::B(next(call, meta)),
        };

        // Get composed transaction
        let res = next(call, meta);
        let upstream = self.upstream.clone();
    
        Either::A(Either::A(Box::new(res.and_then(move |output| {
            let output = output.expect("Output always produced for `MethodCall`");
                
            // TODO [ToDr] Construct RLP and sign.

            let rlp = "0x".into();

            (upstream)(rpc::Call::MethodCall(rpc::MethodCall {
                jsonrpc,
                id,
                method: "eth_sendRawTransaction".into(),
                params: rpc::Params::Array(vec![rlp]),
            }))
        }))))
    }
}
