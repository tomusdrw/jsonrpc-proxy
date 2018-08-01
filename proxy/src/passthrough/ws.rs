//! WebSocket Transport

extern crate websocket;
extern crate serde_json;
extern crate tokio_core;
extern crate jsonrpc_pubsub as pubsub;

use std::collections::HashMap;
use std::sync::{atomic, Arc, Mutex};

use rpc::{
    self,
    futures::{
        self, Future, Sink, Stream,
        sync::{mpsc, oneshot},
    },
};
use self::websocket::{
    ClientBuilder,
    OwnedMessage,
    url::Url,
};
use self::tokio_core::reactor;

type Pending = oneshot::Sender<String>;
type Subscription = mpsc::UnboundedSender<String>;

/// WebSocket transport
#[derive(Debug, Clone)]
pub struct WebSocket {
    id: Arc<atomic::AtomicUsize>,
    url: Url,
    // TODO [ToDr] Get rid of Mutex, rather use `Select` and have another channel that set's up pending requests.
    pending: Arc<Mutex<HashMap<rpc::Id, Pending>>>,
    subscriptions: Arc<Mutex<HashMap<pubsub::SubscriptionId, Subscription>>>,
    write_sender: mpsc::UnboundedSender<OwnedMessage>,
}

impl WebSocket {
    /// Create new WebSocket transport within existing Event Loop.
    pub fn with_event_loop(url: &str, handle: &reactor::Handle) -> Result<Self, String> {
        trace!("Connecting to: {:?}", url);

        let url: Url = url.parse().map_err(|e| format!("{:?}", e))?;
        let pending: Arc<Mutex<HashMap<rpc::Id, Pending>>> = Default::default();
        let subscriptions: Arc<Mutex<HashMap<pubsub::SubscriptionId, Subscription>>> = Default::default();
        let (write_sender, write_receiver) = mpsc::unbounded();

        let ws_future = {
            let pending_ = pending.clone();
            let subscriptions_ = subscriptions.clone();
            let write_sender_ = write_sender.clone();

            ClientBuilder::from_url(&url)
                .async_connect_insecure(handle)
                .map(|(duplex, _)| duplex.split())
                .map_err(|e| format!("{:?}", e))
                .and_then(move |(sink, stream)| {
                    let reader = stream.map_err(|e| format!("{:?}", e)).for_each(move |message| {
                        trace!("Message received: {:?}", message);

                        match message {
                            OwnedMessage::Close(e) => write_sender_
                                .unbounded_send(OwnedMessage::Close(e))
                                .map_err(|e| format!("Error sending close message: {:?}", e)),
                            OwnedMessage::Ping(d) => write_sender_
                                .unbounded_send(OwnedMessage::Pong(d))
                                .map_err(|e| format!("Error sending pong message: {:?}", e)),
                            OwnedMessage::Text(t) => {
                                if let Some(id) = helpers::peek_subscription_id(t.as_bytes()) {
                                    return if let Some(stream) = subscriptions_.lock().unwrap().get(&id) {
                                        // TODO we should most likely cancel the subscription if we detect the other end is unavailable.
                                        stream
                                            .unbounded_send(t)
                                            .map_err(|e| format!("Error sending notification: {:?}", e))
                                    } else {
                                        warn!("Got notification for unknown subscription (id: {:?})", id);
                                        Ok(())
                                    }
                                }

                                if let Some(id) = helpers::peek_id(t.as_bytes()) {
                                    if let Some(request) = pending_.lock().unwrap().remove(&id) {
                                        trace!("Responding to (id: {:?}) with {:?}", id, t);
                                        if let Err(err) = request.send(t) {
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
                        }
                    });

                    let writer = sink
                        .send_all(write_receiver.map_err(|_| websocket::WebSocketError::NoDataAvailable))
                        .map_err(|e| format!("{:?}", e))
                        .map(|_| ());

                    reader.join(writer)
                })
        };

        handle.spawn(ws_future.map(|_| ()).map_err(|err| {
            error!("WebSocketError: {:?}", err);
        }));

        Ok(Self {
            id: Arc::new(atomic::AtomicUsize::new(1)),
            url: url,
            pending,
            subscriptions,
            write_sender,
        })
    }
}

impl super::Transport for WebSocket {
    type Error = String;
    type Future = Box<Future<Item = Option<rpc::Output>, Error = Self::Error> + Send>;

    fn send(&self, call: rpc::Call) -> Self::Future {
        let request = rpc::types::to_string(&call).expect("jsonrpc-core are infallible");

        // TODO [ToDr] Mangle ids per sender.
        let id = helpers::get_id(&call);
        let rx = if let Some(ref id) = id {
            let (tx, rx) = futures::oneshot();
            self.pending.lock().unwrap().insert(id.clone(), tx);
            Some(rx)
        } else {
            None
        };

        // TODO [ToDr] Detect subscriptions here.
        let result = self.write_sender
            .unbounded_send(OwnedMessage::Text(request))
            .map_err(|e| format!("Error sending request: {:?}", e));

        Box::new(
            futures::done(result)
                .and_then(|_| rx.map_err(|e| format!("{:?}", e)))
                .map(|out| out.and_then(|out| serde_json::from_str(&out).ok()))
        )
    }
}

mod helpers {
    use super::*;

    pub fn peek_subscription_id(bytes: &[u8]) -> Option<pubsub::SubscriptionId> {
        // TODO [ToDr] Optimize
        serde_json::from_slice::<rpc::Notification>(bytes)
            .ok()
            .and_then(|notification| {
                println!("notification: {:?}", notification);
                if let Some(rpc::Params::Map(ref map)) = notification.params {
                    map.get("subscription").and_then(|v| pubsub::SubscriptionId::parse_value(v))
                } else {
                    None
                }
            })
    }

    pub fn peek_id(bytes: &[u8]) -> Option<rpc::Id> {
        // TODO [ToDr] Optimize
        serde_json::from_slice::<rpc::Call>(bytes)
            .ok()
            .and_then(|call| get_id(&call))
    }

    pub fn get_id(call: &rpc::Call) -> Option<rpc::Id> {
        match *call {
            rpc::Call::MethodCall(rpc::MethodCall { ref id, .. }) => Some(id.clone()),
            rpc::Call::Notification(_) => None,
            rpc::Call::Invalid(ref id) => Some(id.clone()),
        }
    }
}
