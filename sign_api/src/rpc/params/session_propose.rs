use serde::{Deserialize, Serialize};

use super::{IrnMetadata, Metadata, Namespaces, Relay};

pub(super) const IRN_REQUEST_METADATA: IrnMetadata = IrnMetadata {
    tag: 1100,
    ttl: 300,
    prompt: true,
};

pub(super) const IRN_RESPONSE_METADATA: IrnMetadata = IrnMetadata {
    tag: 1101,
    ttl: 300,
    prompt: false,
};

#[derive(Debug, Serialize, Eq, PartialEq, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Proposer {
    pub public_key: String,
    pub metadata: Metadata,
}

#[derive(Debug, Serialize, PartialEq, Eq, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionProposeRequest {
    pub relays: Vec<Relay>,
    pub proposer: Proposer,
    pub required_namespaces: Namespaces,
}

#[derive(Debug, Serialize, PartialEq, Eq, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionProposeResponse {
    pub relay: Relay,
    pub responder_public_key: String,
}
