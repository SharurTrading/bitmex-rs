//! Consumer-side construction and redaction checks.

use bitmex_client::{ApiCredentials, generated::models::NewOrderV2Body};

#[test]
fn request_models_are_constructible_outside_the_crate() {
    let order = NewOrderV2Body {
        symbol: "XBTUSD".into(),
        side: Some("Buy".into()),
        ord_type: Some("Limit".into()),
        order_qty: Some(1),
        ..Default::default()
    };
    assert_eq!(order.symbol, "XBTUSD");
}

#[test]
fn credentials_and_provider_key_models_redact_debug() {
    let credentials = ApiCredentials::new("fixture-key", "fixture-secret");
    assert!(!format!("{credentials:?}").contains("fixture-secret"));
    let key = bitmex_client::generated::models::ApiKeyGetResponseItem {
        secret: "fixture-provider-secret".into(),
        ..Default::default()
    };
    assert!(!format!("{key:?}").contains("fixture-provider-secret"));
}
