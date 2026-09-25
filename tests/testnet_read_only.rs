//! Ignored, deliberately armed BitMEX Testnet read-only probe.

#![cfg(feature = "live-tests")]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use bitmex_client::{
    ApiCredentials, Client, Environment, Error, OperationError,
    generated::models::{GetInstrumentsQuery, GetOrderV1Query},
    realtime::{Event, Feed, Service, Topic},
};
use std::time::Duration;

fn armed_client() -> Client {
    assert_eq!(
        std::env::var("BITMEX_READ_ONLY_PROBE").ok().as_deref(),
        Some("I_ACCEPT_READ_ONLY_TESTNET")
    );
    let key = std::env::var("BITMEX_TESTNET_API_KEY").expect("set Testnet API key");
    let secret = std::env::var("BITMEX_TESTNET_API_SECRET").expect("set Testnet API secret");
    Client::builder(Environment::Testnet)
        .credentials(ApiCredentials::new(key, secret).expect("valid Testnet credentials"))
        .build()
        .expect("Testnet client")
}

#[tokio::test]
#[ignore = "requires explicit BITMEX_READ_ONLY_PROBE arming"]
async fn testnet_instrument() {
    assert_eq!(
        std::env::var("BITMEX_READ_ONLY_PROBE").ok().as_deref(),
        Some("I_ACCEPT_READ_ONLY_TESTNET")
    );
    let client = Client::builder(Environment::Testnet)
        .build()
        .expect("Testnet client");
    let query = GetInstrumentsQuery {
        count: Some(1),
        ..Default::default()
    };
    let response = client
        .get_instruments(&query)
        .await
        .expect("read-only Testnet response");
    assert!(!response.body.is_empty());
}

#[tokio::test]
#[ignore = "requires explicit arming and injected Testnet credentials"]
async fn testnet_authenticated_rest() {
    let client = armed_client();
    let key = client.api_key_self().await;
    match key {
        Ok(response) => assert!(!response.body.id.is_empty()),
        Err(OperationError::Rejected { status, .. }) => {
            panic!("API key self rejected with HTTP {status}");
        }
        Err(OperationError::Client(Error::Decode(message))) => {
            panic!("API key self decode: {message}");
        }
        Err(OperationError::Client(_)) => panic!("API key self transport or local failure"),
    }
}

#[tokio::test]
#[ignore = "requires explicit arming and injected Testnet credentials"]
async fn testnet_order_query() {
    let client = armed_client();
    let query = GetOrderV1Query {
        count: Some(1),
        ..Default::default()
    };
    match client.get_order_v1(&query).await {
        Ok(_) => {}
        Err(OperationError::Rejected { status, .. }) => {
            panic!("order query rejected with HTTP {status}");
        }
        Err(OperationError::Client(Error::Decode(message))) => {
            panic!("order query decode: {message}");
        }
        Err(OperationError::Client(_)) => panic!("order query transport or local failure"),
    }
}

#[tokio::test]
#[ignore = "requires explicit arming and injected Testnet credentials"]
async fn testnet_authenticated_websocket() {
    let client = armed_client();
    let mut connection = tokio::time::timeout(
        Duration::from_secs(10),
        client.connect_realtime(Service::Primary),
    )
    .await
    .expect("WebSocket connection timeout")
    .expect("WebSocket connection");
    let topic = Topic::new(Feed::Order, None, None).expect("private order topic");
    connection
        .subscribe(&topic)
        .await
        .expect("subscription send");
    for _ in 0..8 {
        let event = tokio::time::timeout(Duration::from_secs(10), connection.next_event())
            .await
            .expect("WebSocket event timeout")
            .expect("WebSocket event");
        match event {
            Event::Subscribed { topic, .. } if topic == "order" => return,
            Event::Rejected { status, .. } => {
                panic!("private subscription rejected with HTTP {status:?}");
            }
            Event::Gap { reason } => {
                panic!("private subscription gap: {reason:?}");
            }
            _ => {}
        }
    }
    panic!("private subscription acknowledgement did not arrive");
}
