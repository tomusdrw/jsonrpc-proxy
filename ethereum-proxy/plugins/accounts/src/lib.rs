//! A simplistic RPC cache.
//!
//! Caches the result of calling the RPC method and clears it
//! depending on the cache eviction policy.

#![warn(missing_docs)]

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{self, AtomicUsize};
use jsonrpc_core::{
    self as rpc,
    futures::{Future, sync::oneshot},
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

/// A middleware intercepting transaction requests and signing them locally.
#[derive(Clone)]
pub struct Middleware {
    secret: Option<SecretKey>,
    upstream: Arc<Upstream>,
    id: Arc<AtomicUsize>,
    lock: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
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
            id: Arc::new(AtomicUsize::new(10_000)),
            lock: Default::default(),
        }
    }
}

const PROOF: &str = "Output always produced for `MethodCall`";

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
        let address = secret.public().address().to_vec();
        let next_id = || {
            let id = self.id.fetch_add(1, atomic::Ordering::SeqCst);
            rpc::Id::Num(id as u64)
        };

        log::trace!("Parsing call: {:?}", call);
        let (jsonrpc, id) = match call {
            rpc::Call::MethodCall(rpc::MethodCall { ref mut method, ref jsonrpc, ref mut id, .. })
                if method == "eth_sendTransaction" || method == "parity_postTransaction" => 
            {
                let orig_id = id.clone();
                *method = "parity_composeTransaction".into();
                *id = next_id();
                (*jsonrpc, orig_id)
            },
            // prepend signing account to the accounts list.
            rpc::Call::MethodCall(rpc::MethodCall { ref mut method, .. })
                if method == "eth_accounts" =>
            {
                let res = next(call, meta).map(|mut output| {
                    if let Some(
                        rpc::Output::Success(ref mut s)
                    ) = output {
                        let rpc::Success { ref mut result, .. } = s;
                        if let rpc::Value::Array(ref mut vec) = result {
                             vec.insert(0, serde_json::to_value(Bytes(address)).unwrap());
                        }
                    }
                    log::debug!("Returning accounts: {:?}", output);
                    output
                });
                return Either::A(Either::A(Box::new(res)))
            },
            _ => return Either::B(next(call, meta)),
        };

        // Acquire lock to make sure we call it sequentially.
        let (tx, previous) = {
            let mut lock = self.lock.lock().unwrap();
            let previous = lock.take();
            let (tx, rx) = oneshot::channel();
            *lock = Some(rx);
            (tx, previous)
        };

        // Get composed transaction
        let chain_id = (self.upstream)(rpc::Call::MethodCall(rpc::MethodCall {
            jsonrpc,
            id: next_id(),
            method: "eth_chainId".into(),
            params: rpc::Params::Array(vec![]),
        }));
        let upstream = self.upstream.clone();
        let upstream2 = upstream.clone();
        let transaction_request = match previous {
            Some(prev) => Either::A(prev.map_err(|_| ()).then(move |_| upstream2(call))),
            None => Either::B(upstream2(call)),
        };
        let res = transaction_request.join(chain_id).and_then(move |(request, chain_id)| {
            log::trace!("Got results, parsing composed transaction and chain_id");
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
                    log::debug!("Got composed: {:?}", result);
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
                    log::debug!("Got chain_id: {:?}", result);
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
            if from.as_bytes() != address {
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
        }).then(move |x| {
            let _ = tx.send(());
            x
        });
        Either::A(Either::A(Box::new(res)))
    }
}

