// Copyright (c) 2018-2020 jsonrpc-proxy contributors.
//
// This file is part of jsonrpc-proxy
// (see https://github.com/tomusdrw/jsonrpc-proxy).
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

//! WebSocket Upstream Transport

#![warn(missing_docs)]

pub mod config;

use jsonrpc_core::futures::{
    self,
    channel::{mpsc, oneshot},
    future::{self, Either},
    Future, FutureExt, StreamExt, TryFutureExt,
};
use std::sync::{atomic, Arc};
use upstream::{
    helpers,
    shared::{PendingKind, Shared},
    Subscription,
};
use websocket::OwnedMessage;

struct WebSocketHandler {
    shared: Arc<Shared>,
    write_sender: mpsc::UnboundedSender<OwnedMessage>,
}

impl WebSocketHandler {
    pub fn process_message(
        &self,
        message: OwnedMessage,
    ) -> impl Future<Output = Result<(), String>> {
        future::ready(match message {
            OwnedMessage::Close(e) => self
                .write_sender
                .unbounded_send(OwnedMessage::Close(e))
                .map_err(|e| format!("Error sending close message: {:?}", e)),
            OwnedMessage::Ping(d) => self
                .write_sender
                .unbounded_send(OwnedMessage::Pong(d))
                .map_err(|e| format!("Error sending pong message: {:?}", e)),
            OwnedMessage::Text(t) => {
                // First check if it's a notification for a subscription
                if let Some(id) = helpers::peek_subscription_id(t.as_bytes()) {
                    return future::ready(self.shared.notify_subscription(&id, t).unwrap_or_else(
                        || {
                            log::warn!("Got notification for unknown subscription (id: {:?})", id);
                            Ok(())
                        },
                    ));
                }

                // then check if it's one of the pending calls
                if let Some(id) = helpers::peek_id(t.as_bytes()) {
                    if let Some((sink, kind)) = self.shared.remove_pending(&id) {
                        match kind {
                            // Just a regular call, don't do anything else.
                            PendingKind::Regular => {}
                            // We have a subscription ID, register subscription.
                            PendingKind::Subscribe(session, unsubscribe) => {
                                let subscription_id = helpers::peek_result(t.as_bytes())
                                    .as_ref()
                                    .and_then(jsonrpc_pubsub::SubscriptionId::parse_value);
                                if let Some(subscription_id) = subscription_id {
                                    self.shared.add_subscription(
                                        subscription_id,
                                        session,
                                        unsubscribe,
                                    );
                                }
                            }
                        }

                        log::trace!("Responding to (id: {:?}) with {:?}", id, t);
                        if let Err(err) = sink.send(t) {
                            log::warn!("Sending a response to deallocated channel: {:?}", err);
                        }
                    } else {
                        log::warn!("Got response for unknown request (id: {:?})", id);
                    }
                } else {
                    log::warn!("Got unexpected notification: {:?}", t);
                }

                Ok(())
            }
            _ => Ok(()),
        })
    }
}

type Spawnable = Box<dyn Future<Output = ()> + Send + Unpin>;

/// A tokio abstraction.
pub trait Spawn: Send + Sync {
    /// Spawn a task in the background.
    fn spawn(&self, ft: Spawnable);
}

impl<F: Fn(Spawnable) + Send + Sync> Spawn for F {
    fn spawn(&self, ft: Spawnable) {
        (*self)(ft)
    }
}

/// WebSocket transport
#[derive(Clone)]
pub struct WebSocket {
    id: Arc<atomic::AtomicUsize>,
    url: url::Url,
    shared: Arc<Shared>,
    spawn: Arc<dyn Spawn>,
    write_sender: mpsc::UnboundedSender<OwnedMessage>,
}

impl std::fmt::Debug for WebSocket {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("WebSocket")
            .field("id", &self.id)
            .field("url", &self.url)
            .field("shared", &self.shared)
            .finish()
    }
}

impl WebSocket {
    /// Create new WebSocket transport within existing Event Loop.
    pub fn new(
        params: Vec<config::Param>,
        spawn_tasks: impl Spawn + 'static,
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
            use futures::compat::Future01CompatExt;
            use futures::TryStreamExt;
            use futures01::{Future, Sink, Stream};

            let handler = WebSocketHandler {
                shared: shared.clone(),
                write_sender: write_sender.clone(),
            };

            let write_receiver = write_receiver
                .map(|msg| {
                    log::trace!("Sending request: {:?}", msg);
                    msg
                })
                .map(|x| Ok(x) as Result<_, websocket::WebSocketError>)
                .compat();
            websocket::ClientBuilder::from_url(&url)
                .async_connect_insecure()
                .map(|(duplex, _)| duplex.split())
                .map_err(|e| format!("{:?}", e))
                .and_then(move |(sink, stream)| {
                    let reader = stream
                        .map_err(|e| format!("{:?}", e))
                        .for_each(move |message| {
                            log::trace!("Message received: {:?}", message);
                            handler.process_message(message).compat()
                        });

                    let writer = sink
                        .send_all(write_receiver)
                        .map_err(|e| format!("{:?}", e))
                        .map(|_| ());

                    reader.join(writer)
                })
                .compat()
        };

        spawn_tasks.spawn(Box::new(
            ws_future
                .map_err(|err| {
                    log::error!("WebSocketError: {:?}", err);
                })
                .map(|_| ()),
        ));

        Ok(Self {
            id: Arc::new(atomic::AtomicUsize::new(1)),
            url,
            shared,
            spawn: Arc::new(spawn_tasks),
            write_sender,
        })
    }

    fn write_and_wait(
        &self,
        call: jsonrpc_core::Call,
        response: Option<oneshot::Receiver<String>>,
    ) -> impl Future<Output = Result<Option<jsonrpc_core::Output>, String>> {
        let request = jsonrpc_core::types::to_string(&call).expect("jsonrpc-core are infallible");
        let result = self
            .write_sender
            .unbounded_send(OwnedMessage::Text(request))
            .map_err(|e| format!("Error sending request: {:?}", e));

        future::ready(result).and_then(|_| match response {
            None => Either::Left(future::ready(Ok(None))),
            Some(res) => res
                .map_ok(|out| serde_json::from_str(&out).ok())
                .map_err(|e| format!("{:?}", e))
                .right_future(),
        })
    }
}

// TODO [ToDr] Might be better to simply have one connection per subscription.
// in case we detect that there is something wrong (i.e. the client disconnected)
// we disconnect from the upstream as well and all the subscriptions are dropped automatically.
impl upstream::Transport for WebSocket {
    type Error = String;
    type Future =
        Box<dyn Future<Output = Result<Option<jsonrpc_core::Output>, Self::Error>> + Send + Unpin>;

    fn send(&self, call: jsonrpc_core::Call) -> Self::Future {
        log::trace!("Calling: {:?}", call);

        // TODO [ToDr] Mangle ids per sender or just ensure atomicity
        let rx = {
            let id = helpers::get_id(&call);
            self.shared.add_pending(id, PendingKind::Regular)
        };

        Box::new(self.write_and_wait(call, rx))
    }

    fn subscribe(
        &self,
        call: jsonrpc_core::Call,
        session: Option<Arc<jsonrpc_pubsub::Session>>,
        subscription: Subscription,
    ) -> Self::Future {
        let session = match session {
            Some(session) => session,
            None => {
                return Box::new(futures::future::err(
                    "Called subscribe without session.".into(),
                ));
            }
        };

        log::trace!("Subscribing to {:?}: {:?}", subscription, call);

        // TODO [ToDr] Mangle ids per sender or just ensure atomicity
        let rx = {
            let ws = self.clone();
            let id = helpers::get_id(&call);
            self.shared.add_pending(
                id,
                PendingKind::Subscribe(
                    session,
                    Box::new(move |subs_id| {
                        // Create unsubscribe request.
                        let call = jsonrpc_core::Call::MethodCall(jsonrpc_core::MethodCall {
                            jsonrpc: Some(jsonrpc_core::Version::V2),
                            id: jsonrpc_core::Id::Num(1),
                            method: subscription.unsubscribe.clone(),
                            params: jsonrpc_core::Params::Array(vec![subs_id.into()]).into(),
                        });
                        let name = subscription.name.clone();
                        let fut = ws
                            .unsubscribe(call, subscription.clone())
                            .map_err(move |e| {
                                log::warn!("Unable to auto-unsubscribe from '{}': {:?}", name, e);
                            })
                            .map(|_| ());

                        ws.spawn.spawn(Box::new(fut));
                    }),
                ),
            )
        };

        Box::new(self.write_and_wait(call, rx))
    }

    fn unsubscribe(&self, call: jsonrpc_core::Call, subscription: Subscription) -> Self::Future {
        log::trace!("Unsubscribing from {:?}: {:?}", subscription, call);

        // Remove the subscription id
        if let Some(subscription_id) = helpers::get_unsubscribe_id(&call) {
            self.shared.remove_subscription(&subscription_id);
        }

        // It's a regular RPC, so just send it
        self.send(call)
    }
}
