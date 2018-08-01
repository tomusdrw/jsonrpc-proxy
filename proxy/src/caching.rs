use rpc::{
    self,
    futures::Future,
    futures::future::Either,
};

use super::Metadata;

#[derive(Debug, Default)]
pub struct Middleware {

}

impl rpc::Middleware<Metadata> for Middleware {
    type Future = rpc::middleware::NoopFuture;
    type CallFuture = rpc::middleware::NoopCallFuture;


    fn on_call<F, X>(&self, call: rpc::Call, meta: Metadata, next: F) -> Either<Self::CallFuture, X> where
        F: FnOnce(rpc::Call, Metadata) -> X + Send,
        X: Future<Item = Option<rpc::Output>, Error = ()> + Send + 'static, 
    {
        match call {
            rpc::Call::MethodCall(rpc::MethodCall { ref method, .. }) => {
                println!("Checking cache for {}", method);
            },
            _ => {},
        }
        Either::B(next(call, meta))
    }
}

#[cfg(test)]
mod tests {

}
