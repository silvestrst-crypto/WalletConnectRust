use anyhow::{Ok, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, PartialEq, Eq, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub description: String,
    pub url: String,
    pub icons: Vec<String>,
    pub name: String,
}

#[derive(Debug, Serialize, PartialEq, Eq, Deserialize, Clone, Default)]
pub struct Relay {
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub data: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Namespace {
    pub chains: Vec<String>,
    pub methods: Vec<String>,
    pub events: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub extensions: Option<Vec<Self>>,
}

impl Namespace {
    pub fn supported(&self, required: &Namespace) -> Result<()> {
        if !required
            .chains
            .iter()
            .all(|item| self.chains.contains(item))
        {
            return Err(anyhow::anyhow!(
                "Chain/chains not supported, actual: {:?}, expected: {:?}",
                self.chains,
                required.chains,
            ));
        }

        if !required
            .methods
            .iter()
            .all(|item| self.methods.contains(item))
        {
            return Err(anyhow::anyhow!(
                "Method/methods not supported, actual: {:?}, expected: {:?}",
                self.methods,
                required.methods,
            ));
        }

        if !required
            .events
            .iter()
            .all(|item| self.events.contains(item))
        {
            return Err(anyhow::anyhow!(
                "Event/events not supported, actual: {:?}, expected: {:?}",
                self.events,
                required.events,
            ));
        }

        match (&self.extensions, &required.extensions) {
            (Some(this), Some(other)) => {
                if !other.iter().all(|item| this.contains(item)) {
                    return Err(anyhow::anyhow!(
                        "Extension/extensions not supported, actual: {:?}, expected: {:?}",
                        this,
                        other,
                    ));
                }
            }
            (Some(other), None) => {
                return Err(anyhow::anyhow!(
                    "Extension/extensions not supported, actual: , expected: {:?}",
                    other,
                ));
            }
            (None, Some(_)) | (None, None) => {}
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Eq, PartialEq, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Namespaces {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub eip155: Option<Namespace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub cosmos: Option<Namespace>,
}

impl Namespaces {
    pub fn supported(&self, required: &Namespaces) -> Result<()> {
        if self.eip155.is_none() && self.cosmos.is_none() {
            return Err(anyhow::anyhow!("No namespaces found"));
        }

        match (&required.eip155, &self.eip155) {
            (Some(other), Some(this)) => {
                return this.supported(other);
            }
            (Some(_), None) => {
                return Err(anyhow::anyhow!("eip155 namespace is required but missing"));
            }
            (None, Some(_)) | (None, None) => {}
        }

        match (&required.cosmos, &self.cosmos) {
            (Some(other), Some(this)) => {
                return this.supported(other);
            }
            (Some(_), None) => {
                return Err(anyhow::anyhow!("Cosmos namespace is required but missing"));
            }
            (None, Some(_)) | (None, None) => {}
        }

        Ok(())
    }
}
