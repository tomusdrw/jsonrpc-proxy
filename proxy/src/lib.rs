#![warn(missing_docs)]

extern crate jsonrpc_core as rpc;

#[macro_use]
extern crate log;

pub mod passthrough;

mod caching;

pub type Metadata = ();
pub type Middleware<T> = (
    caching::Middleware,
    passthrough::Middleware<T>,
);

pub fn handler<T: passthrough::Transport>(transport: T) -> rpc::MetaIoHandler<Metadata, Middleware<T>> {
    rpc::MetaIoHandler::with_middleware((
        caching::Middleware::default(),
        passthrough::Middleware::new(transport),
    ))
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
