//! Shared pieces for building upstream transport.

use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Weak},
};
use parking_lot::{Mutex, RwLock};
use pubsub;
use rpc::{
    self,
    futures::{
        self, Future, Sink,
        sync::oneshot,
    },
};

/// Pending request details
pub type Pending = (oneshot::Sender<String>, PendingKind);
/// A type of unsubscribe function
pub type Unsubscribe = Box<dyn Fn(pubsub::SubscriptionId) + Send>;

/// Pending request type
pub enum PendingKind {
    /// Regular request (RPC -> MethodCall)
    Regular,
    /// Subscribe request (after it's successful we should create a subscription)
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

/// Shared subscription and pending requests manager.
#[derive(Debug, Default)]
pub struct Shared {
    // TODO [ToDr] Get rid of Mutex, rather use `Select` and have another channel that sets up pending requests.
    pending: Mutex<HashMap<rpc::Id, Pending>>,
    // TODO [ToDr] Use (SubscriptionName, SubscriptionId) as key.
    subscriptions: RwLock<HashMap<pubsub::SubscriptionId, Weak<pubsub::Session>>>,
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
            self.pending.lock().insert(id.clone(), (tx, kind));
            Some(rx)
        } else {
            None
        }
    }

    /// Removes a requests from the list of pending requests.
    ///
    /// Most likely the response has been received so we can respond or add a subscription instead.
    pub fn remove_pending(&self, id: &rpc::Id) -> Option<Pending> {
        self.pending.lock().remove(id)
    }

    /// Add a new subscription id and it's correlation with the session.
    pub fn add_subscription(&self, id: pubsub::SubscriptionId, session: Arc<pubsub::Session>, unsubscribe: Unsubscribe) {
        // make sure to send unsubscribe request and remove the subscription.
        let id2 = id.clone();
        session.on_drop(move || unsubscribe(id2));

        trace!("Registered subscription id {:?}", id);
        self.subscriptions.write().insert(id, Arc::downgrade(&session));
    }

    /// Removes a subscription.
    pub fn remove_subscription(&self, id: &pubsub::SubscriptionId) {
        trace!("Removing subscription id {:?}", id);
        self.subscriptions.write().remove(id);
    }

    /// Forwards a notification to given subscription.
    pub fn notify_subscription(&self, id: &pubsub::SubscriptionId, msg: String) 
        -> Option<impl Future<Item = (), Error = String>>
    {
        if let Some(session) = self.subscriptions.read().get(&id) {
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
