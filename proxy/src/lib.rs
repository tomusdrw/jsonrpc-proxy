#![warn(missing_docs)]

extern crate jsonrpc_core as rpc;
extern crate jsonrpc_pubsub as pubsub;
extern crate serde_json;

#[macro_use]
extern crate log;

pub mod passthrough;

mod caching;

pub type Metadata = Option<::std::sync::Arc<pubsub::Session>>;

pub type Middleware<T> = (
    caching::Middleware,
    passthrough::Middleware<T>,
);

pub fn handler<T: passthrough::Transport>(transport: T) -> rpc::MetaIoHandler<Metadata, Middleware<T>> {
    rpc::MetaIoHandler::with_middleware((
        caching::Middleware::default(),
        passthrough::Middleware::new(transport, vec![passthrough::Subscription {
            subscribe: "state_subscribeStorage".into(),
            unsubscribe: "state_unsubscribeStorage".into(),
            name: "state_storage".into(),
        }]),
    ))
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
