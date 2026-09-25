use super::types::{Action, Event, Feed, GapReason, Pool, TableEvent, WireValue};
use crate::{Error, Symbol};
use rust_decimal::Decimal;
use std::collections::BTreeMap;

/// One complete L2 price level.
#[derive(Debug, Clone, PartialEq)]
pub struct BookLevel {
    /// Provider level identity.
    pub id: i64,
    /// Bid or ask side.
    pub side: BookSide,
    /// Exact price.
    pub price: Decimal,
    /// Exact size.
    pub size: Decimal,
}

/// Provider book side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookSide {
    /// Bid level.
    Buy,
    /// Ask level.
    Sell,
}

/// Opt-in, bounded L2 projection for one instrument and pool.
#[derive(Debug)]
pub struct OrderBook {
    symbol: Symbol,
    pool: Option<Pool>,
    max_levels: usize,
    ready: bool,
    levels: BTreeMap<i64, BookLevel>,
}

impl OrderBook {
    /// Create a projection with a strict memory bound.
    pub fn new(symbol: Symbol, pool: Option<Pool>, max_levels: usize) -> Result<Self, Error> {
        if max_levels == 0 {
            return Err(Error::InvalidInput(
                "book level limit must be positive".into(),
            ));
        }
        Ok(Self {
            symbol,
            pool,
            max_levels,
            ready: false,
            levels: BTreeMap::new(),
        })
    }

    /// Whether a complete partial image established current state.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Invalidate state after a continuity gap.
    pub fn invalidate(&mut self) {
        self.ready = false;
        self.levels.clear();
    }

    /// Read levels only when the projection is valid.
    pub fn levels(&self) -> Option<impl Iterator<Item = &BookLevel>> {
        self.ready.then(|| self.levels.values())
    }

    /// Apply a decoded event; any gap invalidates the book.
    pub fn apply_event(&mut self, event: &Event) -> Result<(), GapReason> {
        match event {
            Event::Gap { reason } => {
                self.invalidate();
                Err(*reason)
            }
            Event::Table(table) => self.apply(table),
            _ => Ok(()),
        }
    }

    /// Apply one matching table image or delta.
    pub fn apply(&mut self, event: &TableEvent) -> Result<(), GapReason> {
        let result = self.apply_inner(event);
        if result.is_err() {
            self.invalidate();
        }
        result
    }

    fn apply_inner(&mut self, event: &TableEvent) -> Result<(), GapReason> {
        if !matches!(event.feed, Feed::OrderBookL2 | Feed::OrderBookL2_25) {
            return Ok(());
        }
        if let Some(filter) = &event.filter {
            if filter
                .get("symbol")
                .and_then(WireValue::as_str)
                .is_some_and(|s| s != self.symbol.as_str())
            {
                return Ok(());
            }
            if self.pool.is_some_and(|p| {
                filter
                    .get("pool")
                    .and_then(WireValue::as_str)
                    .is_some_and(|s| s != p.as_str())
            }) {
                return Ok(());
            }
        }
        let has_target_row = event
            .rows
            .iter()
            .any(|row| row.get("symbol").and_then(WireValue::as_str) == Some(self.symbol.as_str()));
        let has_target_filter = event
            .filter
            .as_ref()
            .and_then(|filter| filter.get("symbol"))
            .and_then(WireValue::as_str)
            == Some(self.symbol.as_str());
        if !has_target_row && !has_target_filter {
            if event.action == Action::Partial && event.rows.is_empty() {
                return Err(GapReason::InvalidDelta);
            }
            return Ok(());
        }
        if event.action == Action::Partial {
            self.levels.clear();
            self.ready = true;
        }
        if !self.ready {
            return Err(GapReason::BeforePartial);
        }
        for row in &event.rows {
            let symbol = row
                .get("symbol")
                .and_then(WireValue::as_str)
                .ok_or(GapReason::InvalidDelta)?;
            if symbol != self.symbol.as_str() {
                continue;
            }
            if self.pool.is_some_and(|p| {
                row.get("pool")
                    .and_then(WireValue::as_str)
                    .is_some_and(|s| s != p.as_str())
            }) {
                continue;
            }
            let id = row
                .get("id")
                .and_then(WireValue::as_decimal)
                .and_then(|v| v.to_string().parse::<i64>().ok())
                .ok_or(GapReason::InvalidDelta)?;
            match event.action {
                Action::Delete => {
                    self.levels.remove(&id).ok_or(GapReason::InvalidDelta)?;
                }
                Action::Update => {
                    let level = self.levels.get_mut(&id).ok_or(GapReason::InvalidDelta)?;
                    if let Some(size) = row.get("size").and_then(WireValue::as_decimal) {
                        level.size = size;
                    }
                    if let Some(price) = row.get("price").and_then(WireValue::as_decimal) {
                        level.price = price;
                    }
                }
                Action::Partial | Action::Insert => {
                    let price = row
                        .get("price")
                        .and_then(WireValue::as_decimal)
                        .ok_or(GapReason::InvalidDelta)?;
                    let size = row
                        .get("size")
                        .and_then(WireValue::as_decimal)
                        .ok_or(GapReason::InvalidDelta)?;
                    let side = match row.get("side").and_then(WireValue::as_str) {
                        Some("Buy") => BookSide::Buy,
                        Some("Sell") => BookSide::Sell,
                        _ => return Err(GapReason::InvalidDelta),
                    };
                    if self
                        .levels
                        .insert(
                            id,
                            BookLevel {
                                id,
                                side,
                                price,
                                size,
                            },
                        )
                        .is_some()
                    {
                        return Err(GapReason::InvalidDelta);
                    }
                    if self.levels.len() > self.max_levels {
                        return Err(GapReason::Oversized);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn row(size: &str) -> BTreeMap<String, WireValue> {
        BTreeMap::from([
            ("symbol".into(), WireValue::String("XBTUSD".into())),
            ("id".into(), WireValue::Decimal(Decimal::from(42))),
            ("side".into(), WireValue::String("Buy".into())),
            (
                "price".into(),
                WireValue::Decimal(Decimal::from_str("12345.5").expect("price")),
            ),
            (
                "size".into(),
                WireValue::Decimal(Decimal::from_str(size).expect("size")),
            ),
        ])
    }

    #[test]
    fn book_requires_image_and_invalidates_on_gap() {
        let mut book =
            OrderBook::new(Symbol::new("XBTUSD").expect("symbol"), None, 1).expect("book");
        let delta = TableEvent {
            feed: Feed::OrderBookL2,
            action: Action::Update,
            rows: vec![row("2")],
            keys: None,
            filter: None,
        };
        assert_eq!(book.apply(&delta), Err(GapReason::BeforePartial));
        let image = TableEvent {
            action: Action::Partial,
            ..delta.clone()
        };
        assert!(book.apply(&image).is_ok());
        assert!(book.is_ready());
        assert!(book.apply(&delta).is_ok());
        assert_eq!(
            book.levels().expect("ready").next().map(|level| level.size),
            Some(Decimal::from(2))
        );
        assert_eq!(
            book.apply_event(&Event::Gap {
                reason: GapReason::ConnectionLost
            }),
            Err(GapReason::ConnectionLost)
        );
        assert!(!book.is_ready());
    }

    #[test]
    fn another_symbols_partial_cannot_initialize_this_book() {
        let mut book =
            OrderBook::new(Symbol::new("XBTUSD").expect("symbol"), None, 10).expect("book");
        let mut other = row("2");
        other.insert("symbol".into(), WireValue::String("ETHUSD".into()));
        let image = TableEvent {
            feed: Feed::OrderBookL2,
            action: Action::Partial,
            rows: vec![other],
            keys: None,
            filter: None,
        };
        assert!(book.apply(&image).is_ok());
        assert!(!book.is_ready());
    }
}
