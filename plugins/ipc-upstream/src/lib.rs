//! IPC (JSON-RPC) Upstream Transport

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate cli_params;
extern crate jsonrpc_core as rpc;
extern crate jsonrpc_pubsub as pubsub;
extern crate serde_json;
extern crate tokio;
extern crate tokio_uds;
extern crate upstream;

#[macro_use]
extern crate log;

pub mod config;

use std::{
    sync::{atomic, Arc},
    io::{Error, ErrorKind}
};
use rpc::{
    futures::{
        self, Future, Sink, Stream,
        sync::{mpsc, oneshot},
    },
};
use upstream::{
    Subscription,
    helpers,
    shared::{PendingKind, Shared},
};
use tokio_uds::UnixStream;
use tokio::codec::{Framed, LinesCodec};

struct IpcHandler {
    shared: Arc<Shared>,
}

impl IpcHandler {
    pub fn process_message(&self, message: String) -> impl Future<Item = (), Error = String> {
      use self::futures::{IntoFuture, future::Either};

      // First check if it's a notification for a subscription
      if let Some(id) = helpers::peek_subscription_id(message.as_bytes()) {
          return if let Some(stream) = self.shared.notify_subscription(&id, message) {
              Either::A(stream)
          } else {
              warn!("Got notification for unknown subscription (id: {:?})", id);
              Either::B(Ok(()).into_future())
          }
      }

      // then check if it's one of the pending calls
      if let Some(id) = helpers::peek_id(message.as_bytes()) {
          if let Some((sink, kind)) = self.shared.remove_pending(&id) {
              match kind {
                  // Just a regular call, don't do anything else.
                  PendingKind::Regular => {},
                  // We have a subscription ID, register subscription.
                  PendingKind::Subscribe(session, unsubscribe) => {
                      let subscription_id = helpers::peek_result(message.as_bytes())
                          .as_ref()
                          .and_then(pubsub::SubscriptionId::parse_value);
                      if let Some(subscription_id) = subscription_id {
                          self.shared.add_subscription(subscription_id, session, unsubscribe);
                      }
                  },
              }

              trace!("Responding to (id: {:?}) with {:?}", id, message);
              if let Err(err) = sink.send(message) {
                  warn!("Sending a response to deallocated channel: {:?}", err);
              }
          } else {
              warn!("Got response for unknown request (id: {:?})", id);
          }
      } else {
          warn!("Got unexpected notification: {:?}", message);
      }

      Either::B(Ok(()).into_future())
    }
}

/// IPC transport
#[derive(Debug, Clone)]
pub struct IPC {
    id: Arc<atomic::AtomicUsize>,
    path: String,
    shared: Arc<Shared>,
    write_sender: mpsc::UnboundedSender<String>,
}

impl IPC {
    /// Create new IPC transport within existing Event Loop.
    pub fn new(
        runtime: &mut tokio::runtime::Runtime,
        params: Vec<config::Param>,
    ) -> Result<Self, String> {

        let mut path = "/var/tmp/parity.ipc".to_string();

        for p in params {
            match p {
                config::Param::Path(new_path) => {
                    path = new_path;
                }
            }
        }

        println!("[IPC] Connecting to: {:?}", path);

        let (write_sender, write_receiver) = mpsc::unbounded();
        let shared = Arc::new(Shared::default());

        let handler = IpcHandler {
              shared: shared.clone(),
        };

        runtime.spawn(
          UnixStream::connect(path.clone())
          .and_then(move |client| {
            let (sink, stream) = Framed::new(client, LinesCodec::new()).split();

            let reader = stream.for_each(move |line| {
                handler.process_message(String::from(line)).map_err(|_| Error::new(ErrorKind::Other, "Error processing message"))
            });

            let writer = sink.send_all(
              write_receiver.map_err(|_| Error::new(ErrorKind::Other, "Error in mpsc receiver"))
            );

            writer.join(reader)
          })
          .map(|_| ())
          .map_err(|err| {
              error!("IpcError: {:?}", err);
          })
        );

        Ok(Self {
            id: Arc::new(atomic::AtomicUsize::new(1)),
            path,
            shared,
            write_sender,
        })
    }

    fn write_and_wait(&self, call: rpc::Call, response: Option<oneshot::Receiver<String>>) -> impl Future<Item = Option<rpc::Output>, Error = String>
    {
        let request = rpc::types::to_string(&call).expect("jsonrpc-core are infallible");
        let result = self.write_sender
            .unbounded_send(request)
            .map_err(|e| format!("Error sending request: {:?}", e));

        futures::done(result)
            .and_then(|_| response.map_err(|e| format!("{:?}", e)))
            .map(|out| out.and_then(|out| serde_json::from_str(&out).ok()))
    }
}

// TODO [ToDr] Might be better to simply have one connection per subscription.
// in case we detect that there is something wrong (i.e. the client disconnected)
// we disconnect from the upstream as well and all the subscriptions are dropped automatically.
impl upstream::Transport for IPC {
    type Error = String;
    type Future = Box<Future<Item = Option<rpc::Output>, Error = Self::Error> + Send>;

    fn send(&self, call: rpc::Call) -> Self::Future {
        trace!("Calling: {:?}", call);

        // TODO [ToDr] Mangle ids per sender or just ensure atomicity
        let rx = {
            let id = helpers::get_id(&call);
            self.shared.add_pending(id, PendingKind::Regular)
        };

        Box::new(self.write_and_wait(call, rx))
    }

    fn subscribe(
        &self,
        call: rpc::Call,
        session: Option<Arc<pubsub::Session>>,
        subscription: Subscription,
    ) -> Self::Future {
        let session = match session {
            Some(session) => session,
            None => {
                return Box::new(futures::future::err("Called subscribe without session.".into()));
            }
        };

        trace!("Subscribing to {:?}: {:?}", subscription, call);

        // TODO [ToDr] Mangle ids per sender or just ensure atomicity
        let rx = {
            let ipc = self.clone();
            let id = helpers::get_id(&call);
            self.shared.add_pending(id, PendingKind::Subscribe(session, Box::new(move |subs_id| {
                // Create unsubscribe request.
                let call = rpc::Call::MethodCall(rpc::MethodCall {
                    jsonrpc: Some(rpc::Version::V2),
                    id: rpc::Id::Num(1),
                    method: subscription.unsubscribe.clone(),
                    params: rpc::Params::Array(vec![subs_id.into()]).into(),
                });
                ipc.unsubscribe(call, subscription.clone());
            })))
        };

        Box::new(self.write_and_wait(call, rx))
    }

    fn unsubscribe(
        &self,
        call: rpc::Call,
        subscription: Subscription,
    ) -> Self::Future {

        trace!("Unsubscribing from {:?}: {:?}", subscription, call);

        // Remove the subscription id
        if let Some(subscription_id) = helpers::get_unsubscribe_id(&call) {
            self.shared.remove_subscription(&subscription_id);
        }

        // It's a regular RPC, so just send it
        self.send(call)
    }
}
