# bitmex-rs

An async, provider-native Rust client for BitMEX REST and JSON WebSocket APIs. The Cargo package is `bitmex-client`; the repository is `bitmex-rs`. This is an independent, unofficial library intended as an inner provider client. Strategy, risk, routing, persistence, and UI remain with the consuming application.

The library is pre-release and has only deterministic local verification. No production account or live order has been tested.

## Status

| Surface | Coverage |
| --- | --- |
| Current REST documentation | 141 operations inventoried |
| Callable typed REST methods | 114, each with local success and rejection fixtures |
| Documentation blockers | 27 pages with no published response fields |
| JSON WebSocket topics | 30 named topics on primary and platform sockets |
| Order version | v2 preferred; v1 available for documented compatibility |

The [coverage ledger](docs/coverage.json) and [contract notes](docs/coverage.md) give the exact operation status. The older Explorer Swagger omits the current [v2 order API](https://docs.bitmex.com/api-explorer/order-v2), so current BitMEX endpoint pages are the implementation authority.

## Quick start

```rust
use bitmex_client::{Client, Environment, generated::models::GetInstrumentsQuery};

async fn first_instrument() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder(Environment::Testnet).build()?;
    let query = GetInstrumentsQuery { count: Some(1), ..Default::default() };
    let response = client.get_instruments(&query).await?;
    println!("received {} instruments", response.body.len());
    Ok(())
}
```

Authenticated calls use caller-provided credentials:

```rust
use bitmex_client::{ApiCredentials, Client, Environment};

fn client(key: String, secret: String) -> Result<Client, bitmex_client::Error> {
    let credentials = ApiCredentials::new(key, secret)?;
    Client::builder(Environment::Testnet).credentials(credentials).build()
}
```

`order::NewOrder` provides validated v2 limit, market, stop, and stop-limit constructors. `Client::place_order_v2` submits through `/api/v2/order`. The remaining documented v1 and v2 methods are available under their explicit generated names.

## Safety and realtime behavior

- API signatures cover the exact method, encoded path and query, expiry, and serialized JSON bytes. Credentials are injected and redacted in `Debug`.
- Financial JSON numbers and decimal strings are parsed as exact `rust_decimal::Decimal`; numeric JSON output uses the same exact decimal text.
- REST bodies and WebSocket frames are bounded. Cloned clients share rate admission, cooldown, and mutation fences.
- Mutations are single-attempt. An uncertain outcome fences the affected account or credential scope until the caller reconciles BitMEX state and calls `acknowledge_reconciliation`.
- Realtime connections never silently reconnect. A `Gap` means the caller must obtain a fresh image. The optional L2 book projection invalidates on a gap.

See [SECURITY.md](SECURITY.md) before using credentials or trading methods. No crates.io publication is part of this repository's initial change.
