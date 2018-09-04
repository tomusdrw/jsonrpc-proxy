use pubsub;
use rpc;
use serde_json;

pub fn peek_subscription_id(bytes: &[u8]) -> Option<pubsub::SubscriptionId> {
    // TODO [ToDr] Optimize
    serde_json::from_slice::<rpc::Notification>(bytes)
        .ok()
        .and_then(|notification| {
            if let Some(rpc::Params::Map(ref map)) = notification.params {
                map.get("subscription").and_then(|v| pubsub::SubscriptionId::parse_value(v))
            } else {
                None
            }
        })
}

pub fn peek_result(bytes: &[u8]) -> Option<rpc::Value> {
    // TODO [ToDr] Optimize
    serde_json::from_slice::<rpc::Success>(bytes)
        .ok()
        .map(|res| res.result)
}

pub fn peek_id(bytes: &[u8]) -> Option<rpc::Id> {
    // TODO [ToDr] Optimize
    serde_json::from_slice::<rpc::Call>(bytes)
        .ok()
        .and_then(|call| get_id(&call).cloned())
}

pub fn get_method_name(call: &rpc::Call) -> Option<&str> {
    match *call {
        rpc::Call::MethodCall(rpc::MethodCall { ref method, .. }) => Some(method),
        rpc::Call::Notification(rpc::Notification { ref method, .. }) => Some(method),
        rpc::Call::Invalid(_) => None,
    }
}

pub fn get_id(call: &rpc::Call) -> Option<&rpc::Id> {
    match *call {
        rpc::Call::MethodCall(rpc::MethodCall { ref id, .. }) => Some(id),
        rpc::Call::Notification(_) => None,
        rpc::Call::Invalid(ref id) => Some(id),
    }
}

pub fn get_unsubscribe_id(call: &rpc::Call) -> Option<pubsub::SubscriptionId> {
    match *call {
        rpc::Call::MethodCall(rpc::MethodCall { params: Some(ref params), .. }) |
        rpc::Call::Notification(rpc::Notification { params: Some(ref params), .. }) => match params {
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
