use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Validated provider path identity.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathId(String);

impl PathId {
    /// Validate a nonempty provider identity for a URL path segment.
    pub fn new(value: impl Into<String>) -> Result<Self, crate::Error> {
        let value = value.into();
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(crate::Error::InvalidInput(
                "invalid provider identity".into(),
            ));
        }
        Ok(Self(value))
    }

    /// Return the URL-encoded path segment.
    pub fn encoded(&self) -> String {
        utf8_percent_encode(&self.0, NON_ALPHANUMERIC).to_string()
    }

    /// Return the validated original identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PathId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PathId").field(&self.0).finish()
    }
}

impl Serialize for PathId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PathId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Validated BitMEX instrument symbol, distinct from order identities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Symbol(String);

impl Symbol {
    /// Parse an instrument or index symbol.
    pub fn new(value: impl Into<String>) -> Result<Self, crate::Error> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 64
            || !value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '_' | '-'))
        {
            return Err(crate::Error::InvalidInput("invalid BitMEX symbol".into()));
        }
        Ok(Self(value))
    }
    /// Provider symbol text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Symbol {
    type Error = crate::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<Symbol> for String {
    fn from(value: Symbol) -> Self {
        value.0
    }
}

/// Positive BitMEX account identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct AccountId(i64);
impl AccountId {
    /// Validate the provider account number.
    pub fn new(value: i64) -> Result<Self, crate::Error> {
        if value <= 0 {
            return Err(crate::Error::InvalidInput("invalid account ID".into()));
        }
        Ok(Self(value))
    }
    /// Provider account number.
    pub fn get(self) -> i64 {
        self.0
    }
}
impl TryFrom<i64> for AccountId {
    type Error = crate::Error;
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<AccountId> for i64 {
    fn from(value: AccountId) -> Self {
        value.0
    }
}
