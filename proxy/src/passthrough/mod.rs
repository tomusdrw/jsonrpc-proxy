use rpc::{
    self,
    futures::Future,
    futures::future::Either,
};

use super::Metadata;

pub mod ws;

pub trait Transport: Send + Sync + 'static {
    type Error;
    type Future: Future<Item = Option<rpc::Output>, Error = Self::Error> + Send + 'static;

    fn send(&self, call: rpc::Call) -> Self::Future;
}

#[derive(Debug)]
pub struct Middleware<T> {
    transport: T,
}

impl<T> Middleware<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
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
        // TODO [ToDr] Handle pub-sub (check if method is pub-sub and use different function for transport)
        Either::A(Box::new(
            self.transport.send(request).map_err(|_e| ())
        ))
    }
}
