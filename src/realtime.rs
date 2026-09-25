//! BitMEX JSON WebSocket contracts and bounded connection lifecycle.

mod book;
mod connection;
mod types;

pub use book::{BookLevel, BookSide, OrderBook};
pub use connection::Connection;
pub use types::{
    ALL_FEEDS, Action, Event, Feed, GapReason, Pool, Service, TableEvent, Topic, WireValue,
};
