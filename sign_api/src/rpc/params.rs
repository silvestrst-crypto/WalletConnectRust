pub(super) mod session_delete;
pub(super) mod session_event;
pub(super) mod session_extend;
pub(super) mod session_ping;
pub(super) mod session_propose;
pub(super) mod session_request;
pub(super) mod session_settle;
pub(super) mod session_update;
pub(super) mod shared_types;

pub use session_delete::*;
pub use session_event::*;
pub use session_extend::*;
pub use session_ping::*;
pub use session_propose::*;
pub use session_request::*;
pub use session_settle::*;
pub use session_update::*;
pub use shared_types::*;

use anyhow::Result;
use paste::paste;
pub use serde::{Deserialize, Serialize};
use serde_json::Value;

pub trait RelayProtocolMetadata {
    fn irn_metadata(&self) -> IrnMetadata;
}

pub trait RelayProtocolHelpers {
    type Params;

    fn irn_try_from_tag(value: Value, tag: u32) -> Result<Self::Params>;
}

pub struct IrnMetadata {
    pub tag: u32,
    pub ttl: u64,
    pub prompt: bool,
}

macro_rules! impl_relay_protocol_metadata {
    ($param_type:ty,$meta:ident) => {
        paste! {
            impl RelayProtocolMetadata for $param_type {
                fn irn_metadata(&self) -> IrnMetadata {
                    match self {
                        [<$param_type>]::SessionPropose(_) => session_propose::[<IRN_ $meta:upper _METADATA>],
                        [<$param_type>]::SessionSettle(_) => session_settle::[<IRN_ $meta:upper _METADATA>],
                        [<$param_type>]::SessionUpdate(_) => session_update::[<IRN_ $meta:upper _METADATA>],
                        [<$param_type>]::SessionExtend(_) => session_extend::[<IRN_ $meta:upper _METADATA>],
                        [<$param_type>]::SessionRequest(_) => session_request::[<IRN_ $meta:upper _METADATA>],
                        [<$param_type>]::SessionEvent(_) => session_event::[<IRN_ $meta:upper _METADATA>],
                        [<$param_type>]::SessionDelete(_) => session_delete::[<IRN_ $meta:upper _METADATA>],
                        [<$param_type>]::SessionPing(_) => session_ping::[<IRN_ $meta:upper _METADATA>],
                    }
                }
            }
        }
    }
}

macro_rules! impl_relay_protocol_helpers {
    ($param_type:ty) => {
        paste! {
            impl RelayProtocolHelpers for $param_type {
                type Params = Self;

                fn irn_try_from_tag(value: Value, tag: u32) -> Result<Self::Params> {
                    if tag == session_propose::IRN_RESPONSE_METADATA.tag {
                        Ok(Self::SessionPropose(serde_json::from_value(value)?))
                    } else if tag == session_settle::IRN_RESPONSE_METADATA.tag {
                        Ok(Self::SessionSettle(serde_json::from_value(value)?))
                    } else if tag == session_update::IRN_RESPONSE_METADATA.tag {
                        Ok(Self::SessionUpdate(serde_json::from_value(value)?))
                    } else if tag == session_extend::IRN_RESPONSE_METADATA.tag {
                        Ok(Self::SessionExtend(serde_json::from_value(value)?))
                    } else if tag == session_request::IRN_RESPONSE_METADATA.tag {
                        Ok(Self::SessionRequest(serde_json::from_value(value)?))
                    } else if tag == session_event::IRN_RESPONSE_METADATA.tag {
                        Ok(Self::SessionEvent(serde_json::from_value(value)?))
                    } else if tag == session_delete::IRN_RESPONSE_METADATA.tag {
                        Ok(Self::SessionDelete(serde_json::from_value(value)?))
                    } else if tag == session_ping::IRN_RESPONSE_METADATA.tag {
                        Ok(Self::SessionPing(serde_json::from_value(value)?))
                    } else {
                        anyhow::bail!("tag={tag}, does not match Sign API methods")
                    }
                }
            }
        }
    };
}

#[derive(Debug, Serialize, Eq, Deserialize, Clone, PartialEq)]
#[serde(tag = "method", content = "params")]
pub enum RequestParam {
    #[serde(rename = "wc_sessionPropose")]
    SessionPropose(SessionProposeRequest),
    #[serde(rename = "wc_sessionSettle")]
    SessionSettle(SessionSettleRequest),
    #[serde(rename = "wc_sessionUpdate")]
    SessionUpdate(SessionUpdateRequest),
    #[serde(rename = "wc_sessionExtend")]
    SessionExtend(SessionExtendRequest),
    #[serde(rename = "wc_sessionRequest")]
    SessionRequest(SessionRequestRequest),
    #[serde(rename = "wc_sessionEvent")]
    SessionEvent(SessionEventRequest),
    #[serde(rename = "wc_sessionDelete")]
    SessionDelete(SessionDeleteRequest),
    #[serde(rename = "wc_sessionPing")]
    SessionPing(()),
}
impl_relay_protocol_metadata!(RequestParam, request);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseParam {
    /// A response with a result.
    #[serde(rename = "result")]
    Success(Value),

    /// A response for a failed request.
    #[serde(rename = "error")]
    Err(Value),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseParamSuccess {
    SessionPropose(SessionProposeResponse),
    SessionSettle(bool),
    SessionUpdate(bool),
    SessionExtend(bool),
    SessionRequest(bool),
    SessionEvent(bool),
    SessionDelete(bool),
    SessionPing(bool),
}
impl_relay_protocol_metadata!(ResponseParamSuccess, response);
impl_relay_protocol_helpers!(ResponseParamSuccess);

impl TryFrom<ResponseParamSuccess> for ResponseParam {
    type Error = anyhow::Error;

    fn try_from(value: ResponseParamSuccess) -> Result<Self, Self::Error> {
        Ok(Self::Success(serde_json::to_value(value)?))
    }
}

/// The documentation states that both fields are required.
/// However, on session expiry error, "empty" error is received.
#[derive(Debug, Clone, Eq, Serialize, Deserialize, PartialEq)]
pub struct ErrorParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub code: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseParamError {
    SessionPropose(ErrorParams),
    SessionSettle(ErrorParams),
    SessionUpdate(ErrorParams),
    SessionExtend(ErrorParams),
    SessionRequest(ErrorParams),
    SessionEvent(ErrorParams),
    SessionDelete(ErrorParams),
    SessionPing(ErrorParams),
}
impl_relay_protocol_metadata!(ResponseParamError, response);
impl_relay_protocol_helpers!(ResponseParamError);

impl TryFrom<ResponseParamError> for ResponseParam {
    type Error = anyhow::Error;

    fn try_from(value: ResponseParamError) -> Result<Self, Self::Error> {
        Ok(Self::Err(serde_json::to_value(value)?))
    }
}
