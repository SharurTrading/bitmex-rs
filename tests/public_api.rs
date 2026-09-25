//! Consumer-side construction and redaction checks.

#![allow(clippy::expect_used)]

use bitmex_client::{
    ApiCredentials, ApiKeyListEntry,
    generated::models::{ApiKeySelfResponse, NewOrderV2Body},
};

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
        secret: Some("fixture-provider-secret".into()),
        ..Default::default()
    };
    assert!(!format!("{key:?}").contains("fixture-provider-secret"));
}

#[test]
fn api_key_contract_accepts_live_and_documented_list_shapes() {
    let observed: ApiKeySelfResponse = serde_json::from_str(
        r#"{"id":"fixture","name":"fixture","nonce":1,"userId":1,"cidrs":["0.0.0.0/0"],"permissions":["order"]}"#,
    )
    .expect("observed Testnet shape");
    assert!(observed.secret.is_none());
    assert!(matches!(
        observed.cidrs.as_ref().and_then(|items| items.first()),
        Some(ApiKeyListEntry::Text(_))
    ));
    let documented: ApiKeySelfResponse = serde_json::from_str(
        r#"{"id":"fixture","name":"fixture","nonce":1,"userId":1,"cidrs":[{"cidr":"fixture"}]}"#,
    )
    .expect("documented object shape");
    assert!(matches!(
        documented.cidrs.as_ref().and_then(|items| items.first()),
        Some(ApiKeyListEntry::Object(_))
    ));
}
