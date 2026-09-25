//! Ignored, deliberately armed BitMEX Testnet read-only probe.

#![cfg(feature = "live-tests")]
#![allow(clippy::expect_used)]

use bitmex_client::{Client, Environment, generated::models::GetInstrumentsQuery};

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
