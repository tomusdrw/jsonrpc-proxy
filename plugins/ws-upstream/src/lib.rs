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

use std::{
    sync::{atomic, Arc},
};
use jsonrpc_core::{
    futures::{
        self, Future, TryFutureExt, FutureExt,
        channel::{mpsc, oneshot},
        future::{self, Either},
    },
};
use upstream::{
    Subscription,
    helpers,
    shared::{PendingKind, Shared}, 
};

// impl WebSocketHandler {
//     pub fn process_message(
//         &self,
//         message: OwnedMessage,
//     ) -> impl Future<Output = Result<(), String>> {
//         unimplemented!()
        // Either::Right(future::ready(match message {
        //     OwnedMessage::Close(e) => self.write_sender
        //         .unbounded_send(OwnedMessage::Close(e))
        //         .map_err(|e| format!("Error sending close message: {:?}", e)),
        //     OwnedMessage::Ping(d) => self.write_sender
        //         .unbounded_send(OwnedMessage::Pong(d))
        //         .map_err(|e| format!("Error sending pong message: {:?}", e)),
        //     OwnedMessage::Text(t) => {
        //         // First check if it's a notification for a subscription
        //         if let Some(id) = helpers::peek_subscription_id(t.as_bytes()) {
        //             return if let Some(stream) = self.shared.notify_subscription(&id, t) {
        //                 Either::Left(stream)
        //             } else {
        //                 warn!("Got notification for unknown subscription (id: {:?})", id);
        //                 Either::Right(future::ready(Ok(())))
        //             }
        //         }
        //
        //         // then check if it's one of the pending calls
        //         if let Some(id) = helpers::peek_id(t.as_bytes()) {
        //             if let Some((sink, kind)) = self.shared.remove_pending(&id) {
        //                 match kind {
        //                     // Just a regular call, don't do anything else.
        //                     PendingKind::Regular => {},
        //                     // We have a subscription ID, register subscription.
        //                     PendingKind::Subscribe(session, unsubscribe) => {
        //                         let subscription_id = helpers::peek_result(t.as_bytes())
        //                             .as_ref()
        //                             .and_then(pubsub::SubscriptionId::parse_value);
        //                         if let Some(subscription_id) = subscription_id {
        //                             self.shared.add_subscription(subscription_id, session, unsubscribe);
        //                         }                    
        //                     },
        //                 }
        //
        //                 trace!("Responding to (id: {:?}) with {:?}", id, t);
        //                 if let Err(err) = sink.send(t) {
        //                     warn!("Sending a response to deallocated channel: {:?}", err);
        //                 }
        //             } else {
        //                 warn!("Got response for unknown request (id: {:?})", id);
        //             }
        //         } else {
        //             warn!("Got unexpected notification: {:?}", t);
        //         }
        //
        //         Ok(())
        //     }
        //     _ => Ok(()),
        // }))
//     }
// }

/// WebSocket transport
#[derive(Debug, Clone)]
pub struct WebSocket {
    id: Arc<atomic::AtomicUsize>,
    url: url::Url,
    shared: Arc<Shared>,
    write_sender: mpsc::UnboundedSender<()>,
}


impl WebSocket {
    /// Create new WebSocket transport within existing Event Loop.
    pub fn new(
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

        unimplemented!()
        // let (write_sender, write_receiver) = mpsc::unbounded();
        // let shared = Arc::new(Shared::default());
        //
        // let ws_future = {
        //     let handler = WebSocketHandler {
        //         shared: shared.clone(),
        //         write_sender: write_sender.clone(),
        //     };
        //
        //     ClientBuilder::from_url(&url)
        //         .async_connect_insecure()
        //         .compat()
        //         .map_ok(|(duplex, _)| duplex.split())
        //         .map_err(|e| format!("{:?}", e))
        //         .and_then(move |(sink, stream)| {
        //             let reader = stream
        //                 .compat()
        //                 .map_err(|e| format!("{:?}", e))
        //                 .for_each(move |message| {
        //                     trace!("Message received: {:?}", message);
        //                     handler.process_message(message)
        //                 });
        //
        //             let writer = sink
        //                 .compat()
        //                 .send_all(write_receiver.map_err(|_| websocket::WebSocketError::NoDataAvailable))
        //                 .map_err(|e| format!("{:?}", e))
        //                 .map(|_| ());
        //
        //             future::join(reader, writer)
        //         })
        // };
        //
        // runtime.spawn(ws_future.map(|_| ()).map_err(|err| {
        //     error!("WebSocketError: {:?}", err);
        // }));
        //
        // Ok(Self {
        //     id: Arc::new(atomic::AtomicUsize::new(1)),
        //     url,
        //     shared,
        //     write_sender,
        // })
    }

    // fn write_and_wait(
    //     &self,
    //     call: rpc::Call,
    //     response: Option<oneshot::Receiver<String>>,
    // ) -> impl Future<Output = Result<Option<rpc::Output>, String>> {
    //     let request = rpc::types::to_string(&call).expect("jsonrpc-core are infallible");
    //     let result = self.write_sender
    //         .unbounded_send(OwnedMessage::Text(request))
    //         .map_err(|e| format!("Error sending request: {:?}", e));
    //
    //     future::ready(result)
    //         .and_then(|_| match response {
    //             None => Either::Left(future::ready(Ok(None))),
    //             Some(res) => res
    //                 .map_ok(|out| serde_json::from_str(&out).ok())
    //                 .map_err(|e| format!("{:?}", e))
    //                 .right_future()
    //         })
    // }
}

// TODO [ToDr] Might be better to simply have one connection per subscription.
// in case we detect that there is something wrong (i.e. the client disconnected)
// we disconnect from the upstream as well and all the subscriptions are dropped automatically.
impl upstream::Transport for WebSocket {
    type Error = String;
    type Future = Box<dyn Future<Output = Result<
        Option<jsonrpc_core::Output>,
        Self::Error,
    >> + Send + Unpin>;

    fn send(&self, call: jsonrpc_core::Call) -> Self::Future {
        unimplemented!()
        // trace!("Calling: {:?}", call);
        //
        // // TODO [ToDr] Mangle ids per sender or just ensure atomicity
        // let rx = {
        //     let id = helpers::get_id(&call);
        //     self.shared.add_pending(id, PendingKind::Regular)
        // };
        //
        // Box::new(self.write_and_wait(call, rx))
    }

    fn subscribe(
        &self,
        call: jsonrpc_core::Call,
        session: Option<Arc<jsonrpc_pubsub::Session>>,
        subscription: Subscription,
    ) -> Self::Future {
        unimplemented!()
        // let session = match session {
        //     Some(session) => session,
        //     None => {
        //         return Box::new(futures::future::err("Called subscribe without session.".into()));
        //     }
        // };
        //
        // trace!("Subscribing to {:?}: {:?}", subscription, call);
        //
        // // TODO [ToDr] Mangle ids per sender or just ensure atomicity
        // let rx = {
        //     let ws = self.clone();
        //     let id = helpers::get_id(&call);
        //     self.shared.add_pending(id, PendingKind::Subscribe(session, Box::new(move |subs_id| {
        //         // Create unsubscribe request.
        //         let call = jsonrpc_core::Call::MethodCall(rpc::MethodCall {
        //             jsonrpc: Some(rpc::Version::V2),
        //             id: rpc::Id::Num(1),
        //             method: subscription.unsubscribe.clone(),
        //             params: rpc::Params::Array(vec![subs_id.into()]).into(),
        //         });
        //         if let Err(e) = ws.unsubscribe(call, subscription.clone()).wait() {
        //             warn!("Unable to auto-unsubscribe from '{}': {:?}", subscription.name, e);
        //         }
        //     })))
        // };
        //
        // Box::new(self.write_and_wait(call, rx))
    }

    fn unsubscribe(
        &self,
        call: jsonrpc_core::Call,
        subscription: Subscription,
    ) -> Self::Future {

        log::trace!("Unsubscribing from {:?}: {:?}", subscription, call);
        unimplemented!()

        // // Remove the subscription id
        // if let Some(subscription_id) = helpers::get_unsubscribe_id(&call) {
        //     self.shared.remove_subscription(&subscription_id);
        // }
        //
        // // It's a regular RPC, so just send it
        // self.send(call)
    }
}
