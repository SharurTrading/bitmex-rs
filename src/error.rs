use serde::{Deserialize, Serialize};

/// Local transport, validation, and lifecycle failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A required API key was not configured.
    #[error("API credentials are required")]
    MissingCredentials,
    /// An input cannot be safely sent.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Remote transport failed.
    #[error("transport failure: {0}")]
    Transport(String),
    /// A provider response could not be decoded.
    #[error("provider response decode failure: {0}")]
    Decode(String),
    /// A response exceeded its configured bound.
    #[error("provider response body exceeded configured limit")]
    BodyTooLarge,
    /// Local or provider rate admission denied the request.
    #[error("rate admission denied")]
    RateLimited,
    /// The outcome of a mutation requires provider-state reconciliation.
    #[error("mutation outcome is ambiguous; reconcile provider state for scope {scope}")]
    AmbiguousMutation {
        /// Affected account or credential scope.
        scope: String,
    },
    /// A previous ambiguous mutation still fences this scope.
    #[error("mutation scope {scope} is fenced pending reconciliation")]
    MutationFenced {
        /// Affected account or credential scope.
        scope: String,
    },
    /// The configured endpoint is unsafe.
    #[error("invalid endpoint")]
    InvalidEndpoint,
    /// The system clock cannot produce an API expiry.
    #[error("system clock is unavailable")]
    Clock,
}

/// BitMEX's structured error body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRejection {
    /// Provider error details.
    pub error: ProviderError,
}

/// Provider error name and message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderError {
    /// Provider error class.
    #[serde(default)]
    pub name: Option<String>,
    /// Human-readable provider message.
    #[serde(default)]
    pub message: Option<String>,
    /// Optional provider details.
    #[serde(default)]
    pub details: Option<String>,
    /// Some current endpoint pages document one additional `error` envelope.
    #[serde(default)]
    pub error: Option<Box<ProviderError>>,
}

impl ProviderRejection {
    pub(crate) fn has_detail(&self) -> bool {
        fn present(error: &ProviderError) -> bool {
            error.name.is_some()
                || error.message.is_some()
                || error.error.as_deref().is_some_and(present)
        }
        present(&self.error)
    }
}

/// A typed operation failure.
#[derive(Debug, thiserror::Error)]
pub enum OperationError<R> {
    /// Local or uncertain failure.
    #[error(transparent)]
    Client(#[from] Error),
    /// Definitive provider rejection.
    #[error("BitMEX rejected request with HTTP {status}")]
    Rejected {
        /// HTTP rejection status.
        status: u16,
        /// Typed provider rejection body.
        body: R,
    },
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::ProviderRejection;

    #[test]
    fn nested_provider_rejection_is_typed() {
        let body: ProviderRejection = serde_json::from_str(
            r#"{"error":{"error":{"name":"ValidationError","message":"bad order"}}}"#,
        )
        .expect("nested rejection");
        assert!(body.has_detail());
        assert_eq!(
            body.error.error.as_ref().and_then(|e| e.name.as_deref()),
            Some("ValidationError")
        );
    }
}
