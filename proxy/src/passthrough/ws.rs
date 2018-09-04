//! WebSocket Transport

extern crate websocket;
extern crate tokio_core;

use std::{
    collections::HashMap,
    fmt,
    sync::{atomic, Arc, Weak, Mutex},
};

use pubsub;
use rpc::{
    self,
    futures::{
        self, Future, Sink, Stream,
        sync::{mpsc, oneshot},
    },
};
use serde_json;
use self::websocket::{
    ClientBuilder,
    OwnedMessage,
    url::Url,
};
use self::tokio_core::reactor;
use super::{Subscription, helpers};

type Pending = (oneshot::Sender<String>, PendingKind);
type Unsubscribe = Box<Fn(pubsub::SubscriptionId) + Send>;

enum PendingKind {
    Regular,
    Subscribe(Arc<pubsub::Session>, Unsubscribe),
}

impl fmt::Debug for PendingKind {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PendingKind::Regular => write!(fmt, "Regular"),
            PendingKind::Subscribe(ref session, _) => write!(fmt, "Subscribe({:?})", session),
        }
    }
}

#[derive(Debug)]
struct Shared {
    // TODO [ToDr] Get rid of Mutex, rather use `Select` and have another channel that set's up pending requests.
    pending: Mutex<HashMap<rpc::Id, Pending>>,
    // TODO [ToDr] Use (SubscriptionName, SubscriptionId) as key.
    subscriptions: Arc<Mutex<HashMap<pubsub::SubscriptionId, Weak<pubsub::Session>>>>,
    write_sender: mpsc::UnboundedSender<OwnedMessage>,
}

impl Shared {
    /// Adds a new request to the list of pending requests
    ///
    /// We are awaiting the response for those requests.
    pub fn add_pending(&self, id: Option<&rpc::Id>, kind: PendingKind) 
        -> Option<oneshot::Receiver<String>>
    {
        if let Some(id) = id {
            let (tx, rx) = futures::oneshot();
            self.pending.lock().unwrap().insert(id.clone(), (tx, kind));
            Some(rx)
        } else {
            None
        }
    }

    /// Removes a requests from the list of pending requests.
    ///
    /// Most likely the response has been received so we can respond or add a subscription instead.
    pub fn remove_pending(&self, id: &rpc::Id) -> Option<Pending> {
        self.pending.lock().unwrap().remove(id)
    }

    /// Add a new subscription id and it's correlation with the session.
    pub fn add_subscription(&self, id: pubsub::SubscriptionId, session: Arc<pubsub::Session>, unsubscribe: Unsubscribe) {
        // make sure to send unsubscribe request and remove the subscription.
        let id2 = id.clone();
        session.on_drop(move || unsubscribe(id2));

        trace!("Registered subscription id {:?}", id);
        self.subscriptions.lock().unwrap().insert(id, Arc::downgrade(&session));
    }

    /// Removes a subscription.
    pub fn remove_subscription(&self, id: &pubsub::SubscriptionId) {
        trace!("Removing subscription id {:?}", id);
        self.subscriptions.lock().unwrap().remove(id);
    }

    pub fn notify_subscription(&self, id: &pubsub::SubscriptionId, msg: String) 
        -> Option<impl Future<Item = (), Error = String>>
    {
        if let Some(session) = self.subscriptions.lock().unwrap().get(&id) {
            if let Some(session) = session.upgrade() {
                return Some(session
                    .sender()
                    .send(msg)
                    .map_err(|e| format!("Error sending notification: {:?}", e))
                    .map(|_| ())
                )
            } else {
                error!("Session is not available and subscription was not removed.");
            }
        }

        None
    }
}


struct WebSocketHandler {
    shared: Arc<Shared>,
}

impl WebSocketHandler {
    pub fn process_message(&self, message: OwnedMessage) -> impl Future<Item = (), Error = String> {
        use self::futures::{IntoFuture, future::Either};

        Either::B(match message {
            OwnedMessage::Close(e) => self.shared.write_sender
                .unbounded_send(OwnedMessage::Close(e))
                .map_err(|e| format!("Error sending close message: {:?}", e)),
            OwnedMessage::Ping(d) => self.shared.write_sender
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
}


impl WebSocket {
    /// Create new WebSocket transport within existing Event Loop.
    pub fn with_event_loop(
        url: &str,
        handle: &reactor::Handle,
    ) -> Result<Self, String> {
        trace!("Connecting to: {:?}", url);

        let url = url.parse().map_err(|e| format!("{:?}", e))?;
        let (write_sender, write_receiver) = mpsc::unbounded();
        let shared = Arc::new(Shared {
            pending: Default::default(),
            subscriptions: Default::default(),
            write_sender,
        });

        let ws_future = {
            let handler = WebSocketHandler {
                shared: shared.clone(),
            };

            ClientBuilder::from_url(&url)
                .async_connect_insecure(handle)
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

        handle.spawn(ws_future.map(|_| ()).map_err(|err| {
            error!("WebSocketError: {:?}", err);
        }));

        Ok(Self {
            id: Arc::new(atomic::AtomicUsize::new(1)),
            url,
            shared,
        })
    }

    fn write_and_wait(&self, call: rpc::Call, response: Option<oneshot::Receiver<String>>) -> impl Future<Item = Option<rpc::Output>, Error = String>
    {
        let request = rpc::types::to_string(&call).expect("jsonrpc-core are infallible");
        let result = self.shared.write_sender
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
impl super::Transport for WebSocket {
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
            self.shared.add_pending(id, PendingKind::Subscribe(session, Box::new(move |subscriptionId| {
                // Create unsubscribe request.
                let call = rpc::Call::MethodCall(rpc::MethodCall {
                    jsonrpc: Some(rpc::Version::V2),
                    id: rpc::Id::Num(1),
                    method: subscription.unsubscribe.clone(),
                    params: rpc::Params::Array(vec![subscriptionId.into()]).into(),
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
