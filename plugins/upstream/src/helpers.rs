//! Request parsing helper methods.

use pubsub;
use rpc;
use serde_json;

/// Attempt to peek subscription id from the request given as bytes.
///
/// TODO [ToDr] The implementation should deserialize only subscriptionId part,
/// not the entire `Notification`
pub fn peek_subscription_id(bytes: &[u8]) -> Option<pubsub::SubscriptionId> {
    serde_json::from_slice::<rpc::Notification>(bytes)
        .ok()
        .and_then(|notification| {
            if let rpc::Params::Map(ref map) = notification.params {
                map.get("subscription").and_then(|v| pubsub::SubscriptionId::parse_value(v))
            } else {
                None
            }
        })
}

/// Attempt to peek the result of a successful call.
///
/// TODO [ToDr] The implementation should deserialize only result part,
/// not the entire `rpc::Success`
pub fn peek_result(bytes: &[u8]) -> Option<rpc::Value> {
    serde_json::from_slice::<rpc::Success>(bytes)
        .ok()
        .map(|res| res.result)
}

/// Attempt to peek the id of a call.
///
/// TODO [ToDr] The implementation should deserialize only id part,
/// not the entire `rpc::Call`
pub fn peek_id(bytes: &[u8]) -> Option<rpc::Id> {
    serde_json::from_slice::<rpc::Call>(bytes)
        .ok()
        .and_then(|call| get_id(&call).cloned())
}

/// Extract method name of given call.
pub fn get_method_name(call: &rpc::Call) -> Option<&str> {
    match *call {
        rpc::Call::MethodCall(rpc::MethodCall { ref method, .. }) => Some(method),
        rpc::Call::Notification(rpc::Notification { ref method, .. }) => Some(method),
        rpc::Call::Invalid { .. } => None,
    }
}

/// Get id of given call.
pub fn get_id(call: &rpc::Call) -> Option<&rpc::Id> {
    match *call {
        rpc::Call::MethodCall(rpc::MethodCall { ref id, .. }) => Some(id),
        rpc::Call::Notification(_) => None,
        rpc::Call::Invalid { ref id, .. } => Some(id),
    }
}

/// Extract the first parameter of a call and parse it as subscription id.
pub fn get_unsubscribe_id(call: &rpc::Call) -> Option<pubsub::SubscriptionId> {
    match *call {
        rpc::Call::MethodCall(rpc::MethodCall { ref params, .. }) |
        rpc::Call::Notification(rpc::Notification { ref params, .. }) => match params {
            rpc::Params::Array(ref vec) if !vec.is_empty() => {
                pubsub::SubscriptionId::parse_value(&vec[0])
            },
            _ => {
                warn!("Invalid unsubscribe params: {:?}. Perhaps it's not really an unsubscribe call?", call);
                None
            },
        },
        _ => {
            warn!("Invalid unsubscribe payload: {:?}. Perhaps it's not really an unsubscribe call?", call);
            None
        },
    }
}
