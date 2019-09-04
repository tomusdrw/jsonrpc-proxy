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
use ethsign_transaction::{Bytes, SignTransaction, SignedTransaction, Transaction, U256};

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
            Some(secret) => secret.clone(),
            None => return Either::B(next(call, meta)),
        };

        log::trace!("Parsing call: {:?}", call);
        let (jsonrpc, id) = match call {
            rpc::Call::MethodCall(rpc::MethodCall { ref mut method, ref jsonrpc, ref id, .. })
                if method == "eth_sendTransaction" => {
                *method = "parity_composeTransaction".into();
                (*jsonrpc, id.clone())
            },
            _ => return Either::B(next(call, meta)),
        };

        // Get composed transaction
        let transaction_request = next(call, meta);
        let chain_id = (self.upstream)(rpc::Call::MethodCall(rpc::MethodCall {
            jsonrpc,
            id: id.clone(),
            method: "eth_chain_id".into(),
            params: rpc::Params::Array(vec![]),
        }));
        let upstream = self.upstream.clone();
        let res = transaction_request.join(chain_id).and_then(move |(request, chain_id)| {
            const PROOF: &str = "Output always produced for `MethodCall`";
            let err = |id, msg: &str| {
                Either::A(future::ok(Some(rpc::Output::Failure(rpc::Failure {
                    jsonrpc,
                    id,
                    error: rpc::Error {
                        code: 1.into(),
                        message: msg.into(),
                        data: None,
                    },
                }))))
            };
            let request = match request.expect(PROOF) {
                rpc::Output::Success(rpc::Success { result, .. }) => {
                    log::trace!("Got composed: {:?}", result);
                    match serde_json::from_value::<Transaction>(result) {
                        Ok(tx) => tx,
                        Err(e) => {
                            log::error!("Unable to deserialize transaction request: {:?}", e);
                            return err(id, "Unable to construct transaction")
                        },
                    }
                },
                o => return Either::A(future::ok(Some(o.into()))),
            };
            let chain_id = match chain_id.expect(PROOF) {
                rpc::Output::Success(rpc::Success { result, .. }) => {
                    log::trace!("Got chain_id: {:?}", result);
                    match serde_json::from_value::<U256>(result) {
                        Ok(id) => id.as_u64(),
                        Err(e) => {
                            log::error!("Unable to deserialize transaction request: {:?}", e);
                            return err(id, "Unable to construct transaction")
                        },
                    }
                },
                o => return Either::A(future::ok(Some(o.into()))),
            };
            // Verify from
            let public = secret.public();
            let address = public.address();
            let from = request.from;
            if !from.is_zero() && from.as_bytes() != address {
                log::error!("Expected to send from {:?}, but only support {:?}", from, address);
                return err(id, "Invalid `from` address")
            }
            // Calculate unsigned hash
            let hash = SignTransaction {
                transaction: std::borrow::Cow::Borrowed(&request),
                chain_id,
            }.hash();
            // Sign replay-protected hash.
            let signature = secret.sign(&hash).unwrap();
            // Construct signed RLP
            let signed = SignedTransaction::new(
                std::borrow::Cow::Owned(request),
                chain_id,
                signature.v,
                signature.r,
                signature.s
            );
            let rlp = Bytes(signed.to_rlp());

            Either::B((upstream)(rpc::Call::MethodCall(rpc::MethodCall {
                jsonrpc,
                id,
                method: "eth_sendRawTransaction".into(),
                params: rpc::Params::Array(vec![serde_json::to_value(rlp).unwrap()]),
            })))
        });
    
        Either::A(Either::A(Box::new(res)))
    }
}

