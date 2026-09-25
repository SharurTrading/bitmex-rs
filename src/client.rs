//! Shared REST transport, signing, admission, and mutation reconciliation.

use crate::{Error, OperationError, ProviderRejection};
use futures_util::StreamExt;
use hmac::{Hmac, KeyInit, Mac};
use reqwest::{Method, StatusCode, Url};
use serde::{Serialize, de::DeserializeOwned};
use sha2::Sha256;
use std::{
    collections::{HashSet, VecDeque},
    fmt,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const DEFAULT_MAX_BODY: usize = 64 * 1024 * 1024;
const DEFAULT_MAX_WEBSOCKET_FRAME: usize = 64 * 1024 * 1024;

/// BitMEX production or Testnet environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// BitMEX Testnet.
    Testnet,
    /// BitMEX production.
    Mainnet,
}

impl Environment {
    fn rest(self) -> &'static str {
        match self {
            Self::Testnet => "https://testnet.bitmex.com",
            Self::Mainnet => "https://www.bitmex.com",
        }
    }

    /// The matching JSON WebSocket endpoint for the selected service.
    pub fn websocket(self, platform: bool) -> &'static str {
        match (self, platform) {
            (Self::Mainnet, false) => "wss://ws.bitmex.com/realtime",
            (Self::Mainnet, true) => "wss://www.bitmex.com/realtimePlatform",
            (Self::Testnet, false) => "wss://ws.testnet.bitmex.com/realtime",
            (Self::Testnet, true) => "wss://testnet.bitmex.com/realtimePlatform",
        }
    }
}

/// Injected API key and secret. Its `Debug` output is redacted.
pub struct ApiCredentials {
    key: String,
    secret: String,
}

impl ApiCredentials {
    /// Validate credentials without reading ambient environment variables.
    pub fn new(key: impl Into<String>, secret: impl Into<String>) -> Result<Self, Error> {
        let (key, secret) = (key.into(), secret.into());
        if key.is_empty() || secret.is_empty() || key.chars().any(char::is_control) {
            return Err(Error::InvalidInput("invalid API credentials".into()));
        }
        Ok(Self { key, secret })
    }
}

impl fmt::Debug for ApiCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiCredentials([REDACTED])")
    }
}

/// Provider response with rate and request tracing metadata.
#[derive(Debug, Clone)]
pub struct ApiResponse<T> {
    /// Typed response body.
    pub body: T,
    /// BitMEX request ID, when supplied.
    pub request_id: Option<String>,
    /// Remaining first-layer request budget, when supplied.
    pub rate_remaining: Option<u32>,
}

/// Shareable BitMEX REST client.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    environment: Environment,
    base: Url,
    http: reqwest::Client,
    credentials: Option<ApiCredentials>,
    max_body: usize,
    max_websocket_frame: usize,
    rate: Mutex<Rate>,
    fenced: Mutex<HashSet<String>>,
    in_flight: Mutex<HashSet<String>>,
}

struct Rate {
    minute: VecDeque<Instant>,
    second: VecDeque<Instant>,
    cooldown_until: Option<Instant>,
}

#[derive(Clone, Copy)]
enum BodyEncoding {
    Json,
    Form,
}

/// Client configuration with caller-injected credentials.
pub struct ClientBuilder {
    environment: Environment,
    credentials: Option<ApiCredentials>,
    base_override: Option<Url>,
    max_body: usize,
    max_websocket_frame: usize,
    timeout: Duration,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("environment", &self.inner.environment)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("environment", &self.environment)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Begin a client for Testnet or production. Public data needs no credentials.
    pub fn builder(environment: Environment) -> ClientBuilder {
        ClientBuilder {
            environment,
            credentials: None,
            base_override: None,
            max_body: DEFAULT_MAX_BODY,
            max_websocket_frame: DEFAULT_MAX_WEBSOCKET_FRAME,
            timeout: Duration::from_secs(30),
        }
    }

    /// Acknowledge that the caller reconciled the indicated mutation scope.
    ///
    /// An explicit `targetAccountId` uses `account:<id>`; all other mutations
    /// use `credential`. This method performs no provider query itself.
    pub fn acknowledge_reconciliation(&self, scope: &str) {
        if let Ok(mut fenced) = self.inner.fenced.lock() {
            fenced.remove(scope);
        }
    }

    pub(crate) fn realtime_endpoint(&self, platform: bool) -> &'static str {
        self.inner.environment.websocket(platform)
    }

    pub(crate) fn realtime_frame_limit(&self) -> usize {
        self.inner.max_websocket_frame
    }

    pub(crate) fn realtime_headers(&self) -> Result<Option<(String, String, String)>, Error> {
        let Some(credentials) = self.inner.credentials.as_ref() else {
            return Ok(None);
        };
        let expires = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Clock)?
            .as_secs()
            .checked_add(10)
            .ok_or(Error::Clock)?;
        // The JSON WebSocket guide requires this canonical path for both services.
        let signature = sign(&credentials.secret, "GET", "/realtime", expires, b"")?;
        Ok(Some((
            credentials.key.clone(),
            expires.to_string(),
            signature,
        )))
    }

    pub(crate) fn reserve_realtime_mutation(&self) -> Result<MutationGuard, Error> {
        MutationGuard::reserve(self.inner.clone(), "credential")
    }

    pub(crate) async fn admit_realtime(&self, mutation: bool) -> Result<(), Error> {
        self.admit(mutation, "/api/v1/order/cancelAllAfter").await
    }

    /// Execute one reviewed REST operation without hidden retries.
    pub(crate) async fn execute<T: DeserializeOwned, Q: Serialize, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        query: Option<&Q>,
        body: Option<&B>,
        mutation: bool,
    ) -> Result<ApiResponse<T>, OperationError<ProviderRejection>> {
        self.execute_encoded(method, path, query, body, mutation, BodyEncoding::Json)
            .await
    }

    /// Send a reviewed form-encoded operation, signing exactly the encoded bytes.
    pub(crate) async fn execute_form<T: DeserializeOwned, Q: Serialize, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        query: Option<&Q>,
        body: Option<&B>,
        mutation: bool,
    ) -> Result<ApiResponse<T>, OperationError<ProviderRejection>> {
        self.execute_encoded(method, path, query, body, mutation, BodyEncoding::Form)
            .await
    }

    async fn execute_encoded<T: DeserializeOwned, Q: Serialize, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        query: Option<&Q>,
        body: Option<&B>,
        mutation: bool,
        encoding: BodyEncoding,
    ) -> Result<ApiResponse<T>, OperationError<ProviderRejection>> {
        let query_value = query
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
        let body_value = body
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
        validate_mutation(&method, path, body_value.as_ref())?;
        let scope = if mutation {
            Some(mutation_scope(query_value.as_ref(), body_value.as_ref()))
        } else {
            None
        };
        if mutation && self.inner.credentials.is_none() {
            return Err(Error::MissingCredentials.into());
        }
        self.admit(mutation, path).await?;
        let mut url = self
            .inner
            .base
            .join(path)
            .map_err(|_| Error::InvalidInput("invalid REST path".into()))?;
        if let Some(value) = query_value.as_ref() {
            append_query(&mut url, value)?;
        }
        let body_bytes = match (body, encoding) {
            (Some(body), BodyEncoding::Json) => {
                serde_json::to_vec(body).map_err(|e| Error::InvalidInput(e.to_string()))?
            }
            (Some(_), BodyEncoding::Form) => encode_form(body_value.as_ref())?,
            (None, _) => Vec::new(),
        };
        let mut request = self
            .inner
            .http
            .request(method.clone(), url.clone())
            .header("accept", "application/json");
        if body.is_some() {
            request = request
                .header(
                    "content-type",
                    match encoding {
                        BodyEncoding::Json => "application/json",
                        BodyEncoding::Form => "application/x-www-form-urlencoded",
                    },
                )
                .body(body_bytes.clone());
        }
        if let Some(credentials) = self.inner.credentials.as_ref() {
            let expires = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Error::Clock)?
                .as_secs()
                .checked_add(10)
                .ok_or(Error::Clock)?;
            let signed_path = match url.query() {
                Some(q) => format!("{}?{q}", url.path()),
                None => url.path().to_owned(),
            };
            let signature = sign(
                &credentials.secret,
                method.as_str(),
                &signed_path,
                expires,
                &body_bytes,
            )?;
            request = request
                .header("api-key", &credentials.key)
                .header("api-expires", expires.to_string())
                .header("api-signature", signature);
        }
        let mut guard = scope
            .as_ref()
            .map(|scope| MutationGuard::reserve(self.inner.clone(), scope))
            .transpose()?;
        let response = request.send().await.map_err(|_| match scope.as_ref() {
            Some(scope) => Error::AmbiguousMutation {
                scope: scope.clone(),
            },
            None => Error::Transport("HTTP request failed".into()),
        })?;
        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|h| h.to_str().ok())
            .map(str::to_owned);
        let rate_remaining = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.parse().ok());
        if status == StatusCode::TOO_MANY_REQUESTS {
            self.cooldown(response.headers());
            if let Some(ref mut guard) = guard {
                guard.disarm();
            }
            return Err(Error::RateLimited.into());
        }
        let bytes =
            read_bounded(response, self.inner.max_body)
                .await
                .map_err(|error| match scope.as_ref() {
                    Some(scope) => Error::AmbiguousMutation {
                        scope: scope.clone(),
                    },
                    None => error,
                })?;
        if status.is_success() {
            let body = serde_json::from_slice(&bytes).map_err(|e| match scope.as_ref() {
                Some(scope) => Error::AmbiguousMutation {
                    scope: scope.clone(),
                },
                None => Error::Decode(e.to_string()),
            })?;
            if let Some(ref mut guard) = guard {
                guard.disarm();
            }
            return Ok(ApiResponse {
                body,
                request_id,
                rate_remaining,
            });
        }
        if scope.is_some() && !matches!(status.as_u16(), 400 | 401 | 403 | 404 | 409 | 422) {
            return Err(Error::AmbiguousMutation {
                scope: scope.unwrap_or_default(),
            }
            .into());
        }
        let rejection: ProviderRejection =
            serde_json::from_slice(&bytes).map_err(|e| match scope.as_ref() {
                Some(scope) => Error::AmbiguousMutation {
                    scope: scope.clone(),
                },
                None => Error::Decode(e.to_string()),
            })?;
        if !rejection.has_detail() {
            return Err(match scope.as_ref() {
                Some(scope) => Error::AmbiguousMutation {
                    scope: scope.clone(),
                },
                None => Error::Decode("provider rejection lacked error details".into()),
            }
            .into());
        }
        if let Some(ref mut guard) = guard {
            guard.disarm();
        }
        Err(OperationError::Rejected {
            status: status.as_u16(),
            body: rejection,
        })
    }

    async fn admit(&self, mutation: bool, path: &str) -> Result<(), Error> {
        loop {
            let delay = {
                let mut rate = self
                    .inner
                    .rate
                    .lock()
                    .map_err(|_| Error::Transport("rate state unavailable".into()))?;
                let now = Instant::now();
                while rate
                    .minute
                    .front()
                    .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60))
                {
                    rate.minute.pop_front();
                }
                while rate
                    .second
                    .front()
                    .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(1))
                {
                    rate.second.pop_front();
                }
                let cap = if self.inner.credentials.is_some() {
                    120
                } else {
                    30
                };
                let burst = path.starts_with("/api/v1/order")
                    || path.starts_with("/api/v2/order")
                    || path.starts_with("/api/v1/position");
                let a = rate
                    .cooldown_until
                    .and_then(|t| t.checked_duration_since(now))
                    .unwrap_or_default();
                let b = if rate.minute.len() >= cap {
                    rate.minute
                        .front()
                        .map(|t| Duration::from_secs(60).saturating_sub(now.duration_since(*t)))
                        .unwrap_or_default()
                } else {
                    Duration::ZERO
                };
                let c = if burst && rate.second.len() >= 10 {
                    rate.second
                        .front()
                        .map(|t| Duration::from_secs(1).saturating_sub(now.duration_since(*t)))
                        .unwrap_or_default()
                } else {
                    Duration::ZERO
                };
                let delay = a.max(b).max(c);
                if delay.is_zero() {
                    rate.minute.push_back(now);
                    if burst {
                        rate.second.push_back(now);
                    }
                }
                delay
            };
            if delay.is_zero() {
                return Ok(());
            }
            if mutation {
                return Err(Error::RateLimited);
            }
            tokio::time::sleep(delay).await;
        }
    }

    fn cooldown(&self, headers: &reqwest::header::HeaderMap) {
        let seconds = headers
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1)
            .min(3600);
        if let Ok(mut rate) = self.inner.rate.lock() {
            rate.cooldown_until = Instant::now().checked_add(Duration::from_secs(seconds));
        }
    }
}

impl ClientBuilder {
    /// Inject a BitMEX API key and secret.
    pub fn credentials(mut self, credentials: ApiCredentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Set the maximum accepted REST body size; the default is 64 MiB.
    pub fn max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body = bytes;
        self
    }

    /// Set the maximum WebSocket frame and message size; the default is 64 MiB.
    pub fn max_websocket_frame_bytes(mut self, bytes: usize) -> Self {
        self.max_websocket_frame = bytes;
        self
    }

    /// Override the REST host for an exact loopback test fixture.
    pub fn loopback_rest_url(mut self, url: Url) -> Self {
        self.base_override = Some(url);
        self
    }

    /// Build a shareable client.
    pub fn build(self) -> Result<Client, Error> {
        let base = match self.base_override {
            Some(url) => url,
            None => Url::parse(self.environment.rest()).map_err(|_| Error::InvalidEndpoint)?,
        };
        let valid_remote = base.scheme() == "https"
            && base.as_str().trim_end_matches('/') == self.environment.rest();
        let valid_loopback = base.scheme() == "http"
            && matches!(
                base.host_str(),
                Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
            );
        if !valid_remote && !valid_loopback
            || self.max_body == 0
            || self.max_websocket_frame == 0
            || base.username() != ""
            || base.password().is_some()
        {
            return Err(Error::InvalidEndpoint);
        }
        let http = reqwest::Client::builder()
            .timeout(self.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .map_err(|e| Error::Transport(e.to_string()))?;
        Ok(Client {
            inner: Arc::new(Inner {
                environment: self.environment,
                base,
                http,
                credentials: self.credentials,
                max_body: self.max_body,
                max_websocket_frame: self.max_websocket_frame,
                rate: Mutex::new(Rate {
                    minute: VecDeque::new(),
                    second: VecDeque::new(),
                    cooldown_until: None,
                }),
                fenced: Mutex::new(HashSet::new()),
                in_flight: Mutex::new(HashSet::new()),
            }),
        })
    }
}

fn append_query(url: &mut Url, query: &serde_json::Value) -> Result<(), Error> {
    let object = query
        .as_object()
        .ok_or_else(|| Error::InvalidInput("query must be an object".into()))?;
    let mut pairs = url.query_pairs_mut();
    for (name, value) in object {
        if value.is_null() {
            continue;
        }
        if let Some(values) = value.as_array() {
            for item in values {
                pairs.append_pair(name, &query_text(item));
            }
        } else {
            pairs.append_pair(name, &query_text(value));
        }
    }
    Ok(())
}

fn encode_form(value: Option<&serde_json::Value>) -> Result<Vec<u8>, Error> {
    let object = value
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| Error::InvalidInput("form body must be an object".into()))?;
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in object {
        if !value.is_null() {
            serializer.append_pair(name, &query_text(value));
        }
    }
    Ok(serializer.finish().into_bytes())
}

fn query_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}

fn mutation_scope(query: Option<&serde_json::Value>, body: Option<&serde_json::Value>) -> String {
    for value in [body, query].into_iter().flatten() {
        if let Some(id) = value
            .get("targetAccountId")
            .or_else(|| value.get("account"))
            && !id.is_null()
        {
            return format!("account:{}", query_text(id));
        }
    }
    "credential".into()
}

fn validate_mutation(
    method: &Method,
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<(), Error> {
    let order_path = path == "/api/v1/order" || path == "/api/v2/order";
    if !order_path || *method == Method::GET {
        return Ok(());
    }
    let body = body.ok_or_else(|| Error::InvalidInput("order mutation needs a body".into()))?;
    if *method == Method::POST {
        let symbol = body
            .get("symbol")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if symbol.is_empty() || symbol.chars().any(char::is_control) {
            return Err(Error::InvalidInput("order requires a valid symbol".into()));
        }
        let side = body.get("side").and_then(serde_json::Value::as_str);
        if !matches!(side, Some("Buy" | "Sell")) {
            return Err(Error::InvalidInput("order side must be explicit".into()));
        }
        let order_type = body
            .get("ordType")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::InvalidInput("order type must be explicit".into()))?;
        let instructions = body
            .get("execInst")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let close_without_qty = instructions.split(',').any(|part| part.trim() == "Close")
            && matches!(
                order_type,
                "Stop" | "StopLimit" | "MarketIfTouched" | "LimitIfTouched"
            );
        let qty = body.get("orderQty").and_then(serde_json::Value::as_i64);
        if !close_without_qty && qty.is_none_or(|q| q <= 0) {
            return Err(Error::InvalidInput(
                "order quantity must be positive".into(),
            ));
        }
        if matches!(order_type, "Limit" | "StopLimit" | "LimitIfTouched")
            && !positive_decimal(body.get("price"))
        {
            return Err(Error::InvalidInput(
                "limit order requires a positive price".into(),
            ));
        }
        if matches!(
            order_type,
            "Stop" | "StopLimit" | "MarketIfTouched" | "LimitIfTouched"
        ) && !positive_decimal(body.get("stopPx"))
            && body.get("pegOffsetValue").is_none()
        {
            return Err(Error::InvalidInput(
                "triggered order requires stopPx or pegOffsetValue".into(),
            ));
        }
        if order_type == "Pegged"
            && (body.get("pegPriceType").is_none()
                || body.get("pegOffsetValue").is_none()
                || !instructions.split(',').any(|part| part.trim() == "Fixed"))
        {
            return Err(Error::InvalidInput(
                "pegged order requires peg fields and Fixed instruction".into(),
            ));
        }
    } else if *method == Method::PUT {
        if body.get("orderID").is_none() && body.get("origClOrdID").is_none() {
            return Err(Error::InvalidInput(
                "amend requires orderID or origClOrdID".into(),
            ));
        }
    } else if *method == Method::DELETE
        && body.get("orderID").is_none()
        && body.get("clOrdID").is_none()
    {
        return Err(Error::InvalidInput(
            "cancel requires orderID or clOrdID".into(),
        ));
    }
    Ok(())
}

fn positive_decimal(value: Option<&serde_json::Value>) -> bool {
    value
        .and_then(|v| {
            v.as_number()
                .map(ToString::to_string)
                .or_else(|| v.as_str().map(str::to_owned))
        })
        .and_then(|s| s.parse::<rust_decimal::Decimal>().ok())
        .is_some_and(|decimal| decimal > rust_decimal::Decimal::ZERO)
}

fn sign(
    secret: &str,
    method: &str,
    path: &str,
    expires: u64,
    body: &[u8],
) -> Result<String, Error> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| Error::InvalidInput("invalid signing key".into()))?;
    mac.update(method.as_bytes());
    mac.update(path.as_bytes());
    mac.update(expires.to_string().as_bytes());
    mac.update(body);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

async fn read_bounded(response: reqwest::Response, max: usize) -> Result<Vec<u8>, Error> {
    let mut stream = response.bytes_stream();
    let mut data = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| Error::Transport("HTTP body read failed".into()))?;
        if chunk.len() > max.saturating_sub(data.len()) {
            return Err(Error::BodyTooLarge);
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

pub(crate) struct MutationGuard {
    inner: Arc<Inner>,
    scope: String,
    armed: bool,
}

impl MutationGuard {
    fn reserve(inner: Arc<Inner>, scope: &str) -> Result<Self, Error> {
        let fenced = inner
            .fenced
            .lock()
            .map_err(|_| Error::Transport("fence state unavailable".into()))?;
        if fenced.contains(scope) {
            return Err(Error::MutationFenced {
                scope: scope.into(),
            });
        }
        let mut active = inner
            .in_flight
            .lock()
            .map_err(|_| Error::Transport("mutation state unavailable".into()))?;
        if !active.insert(scope.into()) {
            return Err(Error::MutationFenced {
                scope: scope.into(),
            });
        }
        drop(active);
        drop(fenced);
        Ok(Self {
            inner,
            scope: scope.into(),
            armed: true,
        })
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for MutationGuard {
    fn drop(&mut self) {
        if self.armed
            && let Ok(mut fenced) = self.inner.fenced.lock()
        {
            fenced.insert(self.scope.clone());
        }
        if let Ok(mut active) = self.inner.in_flight.lock() {
            active.remove(&self.scope);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[test]
    fn published_signature_vector() {
        let signature = sign(
            "chNOOS4KvNXR_Xq4k4c9qsfoKWvnDecLATCRlcBwyKDYnWgO",
            "GET",
            "/api/v1/instrument",
            1518064236,
            b"",
        );
        assert_eq!(
            signature.ok().as_deref(),
            Some("c7682d435d0cfe87c16098df34ef2eb5a549d4c5a3c2b1f0f77b8af73423bf00")
        );
    }

    #[test]
    fn generous_defaults_can_be_tightened_and_zero_bounds_are_rejected() {
        let client = Client::builder(Environment::Testnet)
            .build()
            .expect("default client");
        assert_eq!(client.inner.max_body, 64 * 1024 * 1024);
        assert_eq!(client.realtime_frame_limit(), 64 * 1024 * 1024);
        assert!(
            Client::builder(Environment::Testnet)
                .max_body_bytes(0)
                .build()
                .is_err()
        );
        assert!(
            Client::builder(Environment::Testnet)
                .max_websocket_frame_bytes(0)
                .build()
                .is_err()
        );
    }

    #[test]
    fn platform_socket_uses_site_host_and_canonical_realtime_signature() {
        assert_eq!(
            Environment::Testnet.websocket(true),
            "wss://testnet.bitmex.com/realtimePlatform"
        );
        assert_eq!(
            Environment::Mainnet.websocket(true),
            "wss://www.bitmex.com/realtimePlatform"
        );
        let client = Client::builder(Environment::Testnet)
            .credentials(ApiCredentials::new("fixture-key", "fixture-secret").expect("credentials"))
            .build()
            .expect("client");
        let (_, expires, signature) = client.realtime_headers().expect("headers").expect("signed");
        let expected = sign(
            "fixture-secret",
            "GET",
            "/realtime",
            expires.parse().expect("expiry"),
            b"",
        )
        .expect("signature");
        assert_eq!(signature, expected);
    }

    #[tokio::test]
    async fn cancelled_response_fences_clones_but_queries_continue() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fixture bind");
        let address = listener.local_addr().expect("fixture address");
        tokio::spawn(async move {
            let (mut mutation, _) = listener.accept().await.expect("mutation connection");
            let mut buffer = [0_u8; 2048];
            let _ = mutation.read(&mut buffer).await.expect("mutation read");
            drop(mutation);
            let (mut query, _) = listener.accept().await.expect("query connection");
            let _ = query.read(&mut buffer).await.expect("query read");
            query.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").await.expect("query response");
        });
        let url = Url::parse(&format!("http://{address}/")).expect("fixture URL");
        let client = Client::builder(Environment::Testnet)
            .credentials(ApiCredentials::new("key", "secret").expect("credentials"))
            .loopback_rest_url(url)
            .build()
            .expect("client");
        let clone = client.clone();
        let body = serde_json::json!({"orderID":["fixture"]});
        let first = client
            .execute::<serde_json::Value, (), _>(
                Method::DELETE,
                "/api/v2/order",
                None,
                Some(&body),
                true,
            )
            .await;
        assert!(matches!(
            first,
            Err(OperationError::Client(Error::AmbiguousMutation { .. }))
        ));
        let second = clone
            .execute::<serde_json::Value, (), _>(
                Method::DELETE,
                "/api/v2/order",
                None,
                Some(&body),
                true,
            )
            .await;
        assert!(matches!(
            second,
            Err(OperationError::Client(Error::MutationFenced { .. }))
        ));
        let query = clone
            .execute::<serde_json::Value, (), ()>(
                Method::GET,
                "/api/v1/instrument",
                None,
                None,
                false,
            )
            .await;
        assert!(query.is_ok(), "queries must remain available: {query:?}");
        clone.acknowledge_reconciliation("credential");
        assert!(
            !client
                .inner
                .fenced
                .lock()
                .expect("fences")
                .contains("credential")
        );
    }

    #[tokio::test]
    async fn rate_admission_is_shared_and_mutations_fail_immediately() {
        let client = Client::builder(Environment::Testnet)
            .credentials(ApiCredentials::new("key", "secret").expect("credentials"))
            .build()
            .expect("client");
        let now = Instant::now();
        {
            let mut rate = client.inner.rate.lock().expect("rate lock");
            rate.second.extend(std::iter::repeat_n(now, 10));
        }
        let clone = client.clone();
        assert!(matches!(
            clone.admit(true, "/api/v2/order").await,
            Err(Error::RateLimited)
        ));
    }
}
