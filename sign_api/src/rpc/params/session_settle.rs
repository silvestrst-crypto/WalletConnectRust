use serde::{Deserialize, Serialize};

use super::{IrnMetadata, Metadata, Relay};

pub(super) const IRN_REQUEST_METADATA: IrnMetadata = IrnMetadata {
    tag: 1102,
    ttl: 300,
    prompt: false,
};

pub(super) const IRN_RESPONSE_METADATA: IrnMetadata = IrnMetadata {
    tag: 1103,
    ttl: 300,
    prompt: false,
};

#[derive(Debug, Serialize, PartialEq, Eq, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Controller {
    pub public_key: String,
    pub metadata: Metadata,
}

#[derive(Debug, Serialize, PartialEq, Eq, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettleNamespaces {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub eip155: Option<SettleNamespace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub cosmos: Option<SettleNamespace>,
}

#[derive(Debug, Serialize, PartialEq, Eq, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettleNamespace {
    pub accounts: Vec<String>,
    pub methods: Vec<String>,
    pub events: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub extensions: Option<Vec<Self>>,
}

#[derive(Debug, Serialize, PartialEq, Eq, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionSettleRequest {
    pub relay: Relay,
    pub controller: Controller,
    pub namespaces: SettleNamespaces,
    /// uSecs contrary to what documentation says (secs).
    pub expiry: u64,
}
