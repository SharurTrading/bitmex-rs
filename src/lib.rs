//! Async, provider-native client for BitMEX's REST and JSON WebSocket APIs.
//!
//! The caller owns credentials, their storage, and the Tokio runtime.

mod client;
mod decimal_wire;
mod error;
mod ids;
pub mod order;
pub mod realtime;

/// Contracts generated from reviewed BitMEX REST pages.
pub mod generated {
    mod api;
    /// Request and response models.
    pub mod models;
}

pub use client::{ApiCredentials, ApiResponse, Client, ClientBuilder, Environment};
pub use error::{Error, OperationError, ProviderRejection};
pub use ids::{AccountId, PathId, Symbol};

/// Provider-documented but structurally unspecified JSON object.
///
/// Operations whose success contract consists only of this placeholder are
/// recorded as documentation-blocked and do not receive public methods.
pub type UnknownObject = std::collections::BTreeMap<String, serde_json::Value>;

/// API-key list entry: Testnet sends text while the current page declares an unspecified object.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ApiKeyListEntry {
    /// Observed text entry.
    Text(String),
    /// Documented object entry.
    Object(UnknownObject),
}
