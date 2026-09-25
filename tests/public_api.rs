//! Consumer-side construction and redaction checks.

#![allow(clippy::expect_used)]

mod support;

use bitmex_client::{
    ApiCredentials, ApiKeyListEntry, Client, Environment, OperationError,
    generated::models::{ApiKeySelfResponse, ChatNewBody, NewOrderV2Body},
};
use hmac::Mac;

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

#[tokio::test]
async fn form_body_is_encoded_and_signed_as_sent() {
    let (url, received) = support::serve(
        400,
        r#"{"error":{"name":"ValidationError","message":"fixture"}}"#,
    )
    .await;
    let client = Client::builder(Environment::Testnet)
        .credentials(ApiCredentials::new("fixture-key", "fixture-secret").expect("credentials"))
        .loopback_rest_url(url)
        .build()
        .expect("client");
    let body = ChatNewBody {
        channel_id: Some(rust_decimal::Decimal::ONE),
        message: "hello & +/".into(),
    };
    assert!(matches!(
        client.chat_new(&body).await,
        Err(OperationError::Rejected { status: 400, .. })
    ));
    let request = received.await.expect("request recorded");
    let (headers, payload) = request.split_once("\r\n\r\n").expect("request body");
    assert_eq!(payload, "channelID=1&message=hello+%26+%2B%2F");
    assert!(headers.contains("content-type: application/x-www-form-urlencoded"));
    let header = |name: &str| {
        headers
            .lines()
            .find_map(|line| line.strip_prefix(name).map(str::trim))
            .expect("signed request header")
    };
    let expires = header("api-expires:");
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(b"fixture-secret").expect("HMAC");
    mac.update(format!("POST/api/v1/chat{expires}{payload}").as_bytes());
    assert_eq!(
        header("api-signature:"),
        hex::encode(mac.finalize().into_bytes())
    );
}
