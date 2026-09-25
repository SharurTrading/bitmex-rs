use crate::{Error, Symbol};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// BitMEX's two JSON WebSocket services.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    /// Trading and market-data topics.
    Primary,
    /// Platform, chat, and notification topics.
    Platform,
}

/// Liquidity pool for supported market feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pool {
    /// Primary book.
    Primary,
    /// Secondary book.
    Secondary,
    /// Combined book.
    Aggregated,
}
impl Pool {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "Primary",
            Self::Secondary => "Secondary",
            Self::Aggregated => "Aggregated",
        }
    }
    pub(crate) fn from_wire(s: &str) -> Option<Self> {
        match s {
            "Primary" => Some(Self::Primary),
            "Secondary" => Some(Self::Secondary),
            "Aggregated" => Some(Self::Aggregated),
            _ => None,
        }
    }
}

/// One of the thirty documented JSON WebSocket topics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feed {
    /// BitMEX `Funding` table.
    Funding,
    /// BitMEX `Instrument` table.
    Instrument,
    /// BitMEX `Insurance` table.
    Insurance,
    /// BitMEX `Liquidation` table.
    Liquidation,
    /// BitMEX `OrderBookL2_25` table.
    OrderBookL2_25,
    /// BitMEX `OrderBookL2` table.
    OrderBookL2,
    /// BitMEX `OrderBook10` table.
    OrderBook10,
    /// BitMEX `Quote` table.
    Quote,
    /// BitMEX `QuoteBin1m` table.
    QuoteBin1m,
    /// BitMEX `QuoteBin5m` table.
    QuoteBin5m,
    /// BitMEX `QuoteBin1h` table.
    QuoteBin1h,
    /// BitMEX `QuoteBin1d` table.
    QuoteBin1d,
    /// BitMEX `Settlement` table.
    Settlement,
    /// BitMEX `Trade` table.
    Trade,
    /// BitMEX `TradeBin1m` table.
    TradeBin1m,
    /// BitMEX `TradeBin5m` table.
    TradeBin5m,
    /// BitMEX `TradeBin1h` table.
    TradeBin1h,
    /// BitMEX `TradeBin1d` table.
    TradeBin1d,
    /// BitMEX `Affiliate` table.
    Affiliate,
    /// BitMEX `Execution` table.
    Execution,
    /// BitMEX `Order` table.
    Order,
    /// BitMEX `Margin` table.
    Margin,
    /// BitMEX `Position` table.
    Position,
    /// BitMEX `Transact` table.
    Transact,
    /// BitMEX `Wallet` table.
    Wallet,
    /// BitMEX `Announcement` table.
    Announcement,
    /// BitMEX `Chat` table.
    Chat,
    /// BitMEX `Connected` table.
    Connected,
    /// BitMEX `PublicNotifications` table.
    PublicNotifications,
    /// BitMEX `PrivateNotifications` table.
    PrivateNotifications,
}

/// Pinned current feed inventory.
pub const ALL_FEEDS: [Feed; 30] = [
    Feed::Funding,
    Feed::Instrument,
    Feed::Insurance,
    Feed::Liquidation,
    Feed::OrderBookL2_25,
    Feed::OrderBookL2,
    Feed::OrderBook10,
    Feed::Quote,
    Feed::QuoteBin1m,
    Feed::QuoteBin5m,
    Feed::QuoteBin1h,
    Feed::QuoteBin1d,
    Feed::Settlement,
    Feed::Trade,
    Feed::TradeBin1m,
    Feed::TradeBin5m,
    Feed::TradeBin1h,
    Feed::TradeBin1d,
    Feed::Affiliate,
    Feed::Execution,
    Feed::Order,
    Feed::Margin,
    Feed::Position,
    Feed::Transact,
    Feed::Wallet,
    Feed::Announcement,
    Feed::Chat,
    Feed::Connected,
    Feed::PublicNotifications,
    Feed::PrivateNotifications,
];

impl Feed {
    /// Provider topic name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Funding => "funding",
            Self::Instrument => "instrument",
            Self::Insurance => "insurance",
            Self::Liquidation => "liquidation",
            Self::OrderBookL2_25 => "orderBookL2_25",
            Self::OrderBookL2 => "orderBookL2",
            Self::OrderBook10 => "orderBook10",
            Self::Quote => "quote",
            Self::QuoteBin1m => "quoteBin1m",
            Self::QuoteBin5m => "quoteBin5m",
            Self::QuoteBin1h => "quoteBin1h",
            Self::QuoteBin1d => "quoteBin1d",
            Self::Settlement => "settlement",
            Self::Trade => "trade",
            Self::TradeBin1m => "tradeBin1m",
            Self::TradeBin5m => "tradeBin5m",
            Self::TradeBin1h => "tradeBin1h",
            Self::TradeBin1d => "tradeBin1d",
            Self::Affiliate => "affiliate",
            Self::Execution => "execution",
            Self::Order => "order",
            Self::Margin => "margin",
            Self::Position => "position",
            Self::Transact => "transact",
            Self::Wallet => "wallet",
            Self::Announcement => "announcement",
            Self::Chat => "chat",
            Self::Connected => "connected",
            Self::PublicNotifications => "publicNotifications",
            Self::PrivateNotifications => "privateNotifications",
        }
    }
    /// Required socket service.
    pub fn service(self) -> Service {
        match self {
            Self::Announcement
            | Self::Chat
            | Self::Connected
            | Self::PublicNotifications
            | Self::PrivateNotifications => Service::Platform,
            _ => Service::Primary,
        }
    }
    /// Whether a signed API key is required.
    pub fn requires_auth(self) -> bool {
        matches!(
            self,
            Self::Affiliate
                | Self::Execution
                | Self::Order
                | Self::Margin
                | Self::Position
                | Self::Transact
                | Self::Wallet
                | Self::PrivateNotifications
        )
    }
    /// Whether the feed accepts an explicit liquidity pool.
    pub fn supports_pool(self) -> bool {
        matches!(
            self,
            Self::OrderBookL2
                | Self::OrderBookL2_25
                | Self::OrderBook10
                | Self::Trade
                | Self::TradeBin1m
                | Self::TradeBin5m
                | Self::TradeBin1h
                | Self::TradeBin1d
                | Self::Quote
                | Self::QuoteBin1m
                | Self::QuoteBin5m
                | Self::QuoteBin1h
                | Self::QuoteBin1d
        )
    }
    pub(crate) fn from_wire(s: &str) -> Option<Self> {
        ALL_FEEDS.iter().copied().find(|feed| feed.as_str() == s)
    }
}

/// Validated subscription target.
#[derive(Debug, Clone)]
pub struct Topic {
    /// Feed family.
    feed: Feed,
    /// Optional instrument filter.
    symbol: Option<Symbol>,
    /// Optional liquidity pool.
    pool: Option<Pool>,
}
impl Topic {
    /// Validate feed and pool compatibility.
    pub fn new(feed: Feed, symbol: Option<Symbol>, pool: Option<Pool>) -> Result<Self, Error> {
        if symbol.as_ref().is_some_and(|s| s.as_str().contains(':')) {
            return Err(Error::InvalidInput(
                "WebSocket symbol cannot contain a colon".into(),
            ));
        }
        if pool.is_some() && (!feed.supports_pool() || symbol.is_none()) {
            return Err(Error::InvalidInput(
                "pool requires a pool-capable feed and symbol".into(),
            ));
        }
        Ok(Self { feed, symbol, pool })
    }
    /// Selected feed.
    pub fn feed(&self) -> Feed {
        self.feed
    }
    /// Optional validated symbol.
    pub fn symbol(&self) -> Option<&Symbol> {
        self.symbol.as_ref()
    }
    /// Requested pool, when specified.
    pub fn pool(&self) -> Option<Pool> {
        self.pool
    }
    pub(crate) fn wire(&self) -> String {
        let mut s = self.feed.as_str().to_owned();
        if let Some(symbol) = &self.symbol {
            s.push(':');
            s.push_str(symbol.as_str());
        }
        if let Some(pool) = self.pool {
            s.push(':');
            s.push_str(pool.as_str());
        }
        s
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod topic_tests {
    use super::*;

    #[test]
    fn every_pinned_feed_has_the_expected_service_and_auth_scope() {
        let inventory: serde_json::Value =
            serde_json::from_str(include_str!("../../spec/official/ws-topics.json"))
                .expect("pinned WebSocket topics");
        let names = |group: &str| {
            inventory[group]
                .as_array()
                .expect("topic group")
                .iter()
                .map(|name| name.as_str().expect("topic name"))
                .collect::<Vec<_>>()
        };
        let primary_public = names("primary_public");
        let primary_private = names("primary_private");
        let platform = names("platform");
        assert_eq!(
            ALL_FEEDS.len(),
            primary_public.len() + primary_private.len() + platform.len()
        );
        for feed in ALL_FEEDS {
            let name = feed.as_str();
            let (service, private) = if primary_public.contains(&name) {
                (Service::Primary, false)
            } else if primary_private.contains(&name) {
                (Service::Primary, true)
            } else {
                assert!(platform.contains(&name), "unlisted topic: {name}");
                (Service::Platform, name == "privateNotifications")
            };
            assert_eq!(feed.service(), service, "{name}");
            assert_eq!(feed.requires_auth(), private, "{name}");
            assert!(Topic::new(feed, None, None).is_ok(), "{name}");
        }
    }

    #[test]
    fn pool_subscription_uses_documented_third_token() {
        let symbol = Symbol::new("XBTUSD").expect("symbol");
        let topic =
            Topic::new(Feed::OrderBookL2_25, Some(symbol), Some(Pool::Secondary)).expect("topic");
        assert_eq!(topic.wire(), "orderBookL2_25:XBTUSD:Secondary");
        let timeframe = Symbol::new("XBT:quarterly").expect("symbol");
        assert!(Topic::new(Feed::OrderBookL2, Some(timeframe), None).is_err());
    }
}

/// Exact recursive WebSocket field value, including decimal JSON numbers.
#[derive(Debug, Clone, PartialEq)]
pub enum WireValue {
    /// JSON null.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Exact JSON number.
    Decimal(Decimal),
    /// String value.
    String(String),
    /// Nested array.
    Array(Vec<Self>),
    /// Nested object.
    Object(BTreeMap<String, Self>),
}
impl TryFrom<Value> for WireValue {
    type Error = Error;
    fn try_from(value: Value) -> Result<Self, Error> {
        Ok(match value {
            Value::Null => Self::Null,
            Value::Bool(v) => Self::Bool(v),
            Value::Number(n) => Self::Decimal(
                crate::decimal_wire::parse_decimal(&n.to_string())
                    .map_err(|e| Error::Decode(e.to_string()))?,
            ),
            Value::String(s) => Self::String(s),
            Value::Array(items) => Self::Array(
                items
                    .into_iter()
                    .map(Self::try_from)
                    .collect::<Result<_, _>>()?,
            ),
            Value::Object(fields) => Self::Object(
                fields
                    .into_iter()
                    .map(|(k, v)| Ok((k, Self::try_from(v)?)))
                    .collect::<Result<_, Error>>()?,
            ),
        })
    }
}
impl WireValue {
    /// Return a numeric value when this is a decimal.
    pub fn as_decimal(&self) -> Option<Decimal> {
        if let Self::Decimal(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    /// Return a string value when this is a string.
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

/// BitMEX table action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Complete initial table image.
    Partial,
    /// New row.
    Insert,
    /// Changed columns on an existing row.
    Update,
    /// Removed row.
    Delete,
}

/// One table image or delta.
#[derive(Debug, Clone)]
pub struct TableEvent {
    /// Feed family.
    pub feed: Feed,
    /// Image or delta action.
    pub action: Action,
    /// Rows with exact decimal values.
    pub rows: Vec<BTreeMap<String, WireValue>>,
    /// Provider key columns, present on images.
    pub keys: Option<Vec<String>>,
    /// Subscription filter echo.
    pub filter: Option<BTreeMap<String, WireValue>>,
}

/// Explicit continuity failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapReason {
    /// Malformed JSON or table structure.
    Malformed,
    /// Configured size bound was exceeded.
    Oversized,
    /// Socket generation ended.
    ConnectionLost,
    /// Delta arrived before its image.
    BeforePartial,
    /// Unknown provider table.
    UnknownTable,
    /// Delta cannot be safely applied.
    InvalidDelta,
}

/// Decoded WebSocket event.
#[derive(Debug, Clone)]
pub enum Event {
    /// Welcome frame.
    Welcome,
    /// Subscription accepted.
    Subscribed {
        /// Acknowledged topic.
        topic: String,
        /// Resolved pool when supplied by BitMEX.
        pool: Option<Pool>,
    },
    /// Unsubscription accepted.
    Unsubscribed {
        /// Removed topic.
        topic: String,
    },
    /// Typed table action.
    Table(TableEvent),
    /// The caller must obtain a new image.
    Gap {
        /// Why a fresh image is required.
        reason: GapReason,
    },
    /// Provider command rejection.
    Rejected {
        /// Provider status when supplied.
        status: Option<u16>,
        /// Provider message.
        message: String,
    },
    /// Dead-man switch acknowledgment.
    DeadManAcknowledged {
        /// Provider cancel deadline, if represented as text.
        cancel_time: Option<String>,
    },
    /// Ping response.
    Pong,
}
