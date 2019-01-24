//! WebSocket Upstream Transport

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

extern crate cli_params;
extern crate jsonrpc_core as rpc;
extern crate jsonrpc_pubsub as pubsub;
extern crate serde_json;
extern crate websocket;
extern crate tokio;
extern crate upstream;

#[macro_use]
extern crate log;

pub mod config;

use std::{
    sync::{atomic, Arc},
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
use websocket::{
    ClientBuilder,
    OwnedMessage,
    url::Url,
};

struct WebSocketHandler {
    shared: Arc<Shared>,
    write_sender: mpsc::UnboundedSender<OwnedMessage>,
}

impl WebSocketHandler {
    pub fn process_message(&self, message: OwnedMessage) -> impl Future<Item = (), Error = String> {
        use self::futures::{IntoFuture, future::Either};

        Either::B(match message {
            OwnedMessage::Close(e) => self.write_sender
                .unbounded_send(OwnedMessage::Close(e))
                .map_err(|e| format!("Error sending close message: {:?}", e)),
            OwnedMessage::Ping(d) => self.write_sender
                .unbounded_send(OwnedMessage::Pong(d))
                .map_err(|e| format!("Error sending pong message: {:?}", e)),
            OwnedMessage::Text(t) => {
                // First check if it's a notification for a subscription
                if let Some(id) = helpers::peek_subscription_id(t.as_bytes()) {
                    return if let Some(stream) = self.shared.notify_subscription(&id, t) {
                        Either::A(stream)
                    } else {
                        warn!("Got notification for unknown subscription (id: {:?})", id);
                        Either::B(Ok(()).into_future())
                    }
                }

                // then check if it's one of the pending calls
                if let Some(id) = helpers::peek_id(t.as_bytes()) {
                    if let Some((sink, kind)) = self.shared.remove_pending(&id) {
                        match kind {
                            // Just a regular call, don't do anything else.
                            PendingKind::Regular => {},
                            // We have a subscription ID, register subscription.
                            PendingKind::Subscribe(session, unsubscribe) => {
                                let subscription_id = helpers::peek_result(t.as_bytes())
                                    .as_ref()
                                    .and_then(pubsub::SubscriptionId::parse_value);
                                if let Some(subscription_id) = subscription_id {
                                    self.shared.add_subscription(subscription_id, session, unsubscribe);
                                }                    
                            },
                        }

                        trace!("Responding to (id: {:?}) with {:?}", id, t);
                        if let Err(err) = sink.send(t) {
                            warn!("Sending a response to deallocated channel: {:?}", err);
                        }
                    } else {
                        warn!("Got response for unknown request (id: {:?})", id);
                    }
                } else {
                    warn!("Got unexpected notification: {:?}", t);
                }

                Ok(())
            }
            _ => Ok(()),
        }.into_future())
    }
}

/// WebSocket transport
#[derive(Debug, Clone)]
pub struct WebSocket {
    id: Arc<atomic::AtomicUsize>,
    url: Url,
    shared: Arc<Shared>,
    write_sender: mpsc::UnboundedSender<OwnedMessage>,
}


impl WebSocket {
    /// Create new WebSocket transport within existing Event Loop.
    pub fn new(
        runtime: &mut tokio::runtime::Runtime,
        params: Vec<config::Param>,
    ) -> Result<Self, String> {

        let mut url = "ws://127.0.0.1:9944".parse().expect("Valid address given.");

        for p in params {
            match p {
                config::Param::Url(new_url) => {
                    url = new_url;
                }
            }
        }

        println!("[WS] Connecting to: {:?}", url);

        let (write_sender, write_receiver) = mpsc::unbounded();
        let shared = Arc::new(Shared::default());

        let ws_future = {
            let handler = WebSocketHandler {
                shared: shared.clone(),
                write_sender: write_sender.clone(),
            };

            ClientBuilder::from_url(&url)
                .async_connect_insecure()
                .map(|(duplex, _)| duplex.split())
                .map_err(|e| format!("{:?}", e))
                .and_then(move |(sink, stream)| {
                    let reader = stream
                        .map_err(|e| format!("{:?}", e))
                        .for_each(move |message| {
                            trace!("Message received: {:?}", message);
                            handler.process_message(message)
                        });

                    let writer = sink
                        .send_all(write_receiver.map_err(|_| websocket::WebSocketError::NoDataAvailable))
                        .map_err(|e| format!("{:?}", e))
                        .map(|_| ());

                    reader.join(writer)
                })
        };

        runtime.spawn(ws_future.map(|_| ()).map_err(|err| {
            error!("WebSocketError: {:?}", err);
        }));

        Ok(Self {
            id: Arc::new(atomic::AtomicUsize::new(1)),
            url,
            shared,
            write_sender,
        })
    }

    fn write_and_wait(&self, call: rpc::Call, response: Option<oneshot::Receiver<String>>) -> impl Future<Item = Option<rpc::Output>, Error = String>
    {
        let request = rpc::types::to_string(&call).expect("jsonrpc-core are infallible");
        let result = self.write_sender
            .unbounded_send(OwnedMessage::Text(request))
            .map_err(|e| format!("Error sending request: {:?}", e));

        futures::done(result)
            .and_then(|_| response.map_err(|e| format!("{:?}", e)))
            .map(|out| out.and_then(|out| serde_json::from_str(&out).ok()))
    }
}

// TODO [ToDr] Might be better to simply have one connection per subscription.
// in case we detect that there is something wrong (i.e. the client disconnected)
// we disconnect from the upstream as well and all the subscriptions are dropped automatically.
impl upstream::Transport for WebSocket {
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
            let ws = self.clone();
            let id = helpers::get_id(&call);
            self.shared.add_pending(id, PendingKind::Subscribe(session, Box::new(move |subs_id| {
                // Create unsubscribe request.
                let call = rpc::Call::MethodCall(rpc::MethodCall {
                    jsonrpc: Some(rpc::Version::V2),
                    id: rpc::Id::Num(1),
                    method: subscription.unsubscribe.clone(),
                    params: rpc::Params::Array(vec![subs_id.into()]).into(),
                });
                ws.unsubscribe(call, subscription.clone());
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
