# bitmex-rs

[![CI](https://github.com/SharurTrading/bitmex-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/SharurTrading/bitmex-rs/actions/workflows/ci.yml)
[![MSRV: Rust 1.95.0](https://img.shields.io/badge/MSRV-Rust%201.95.0-blue.svg?logo=rust)](rust-toolchain.toml)
[![license: MIT-0](https://img.shields.io/badge/license-MIT--0-blue.svg)](LICENSE)
[![current REST: 114/141 callable](https://img.shields.io/badge/current%20REST-114%2F141%20callable-yellow.svg)](docs/coverage.json)
[![JSON WebSocket: 30 topics](https://img.shields.io/badge/JSON%20WebSocket-30%20topics-blue.svg)](spec/official/ws-topics.json)

> [!WARNING]
> **Pre-release:** verification uses deterministic local fixtures. No authenticated Testnet or
> Mainnet account and no live order has been tested. Validate the client and your reconciliation
> flow independently before using it for live trading.

An async, provider-native Rust client for the [BitMEX REST API](https://docs.bitmex.com/api-explorer)
and [JSON WebSocket API](https://www.bitmex.com/app/wsAPI). The Cargo package is `bitmex-client`,
the Rust import is `bitmex_client`, and `-rs` belongs to the repository name. This is an
independent, unofficial client, unaffiliated with BitMEX. It is designed as an inner provider
client; strategy, risk, routing, portfolio, persistence, and UI belong to its caller.

The project is licensed under [MIT No Attribution (MIT-0)](LICENSE). Its minimum supported Rust
version is **1.95.0**.

## Status

| Surface | Coverage |
| --- | --- |
| Current REST documentation | 141 operations inventoried |
| Callable typed REST methods | 114, each with local success and rejection fixtures |
| Documentation blockers | 27 pages with no published response fields |
| JSON WebSocket topics | 30 named topics on primary and platform sockets |
| Order version | v2 preferred; v1 available for documented compatibility |

The current BitMEX endpoint pages, reviewed on 2026-09-25, are the REST authority. The older
Explorer Swagger lists 120 operations and omits the current
[v2 order API](https://docs.bitmex.com/api-explorer/order-v2). The [coverage ledger](docs/coverage.json)
and [contract notes](docs/coverage.md) name every operation and its source. The 27 blocked pages
publish an empty `200` object without enough fields to build a safe typed success contract; this
crate does **not** claim 100% callable REST coverage. WebSocket rows use exact recursive wire
values, rather than separate static structs for every feed.

## Installation

The package is not published to crates.io. Consume a reviewed local checkout:

```toml
[dependencies]
bitmex-client = { path = "../bitmex-rs" }
tokio = { version = "1", features = ["macros", "rt"] }
```

## Quick start

```rust
use bitmex_client::{Client, Environment, generated::models::GetInstrumentsQuery};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder(Environment::Testnet).build()?;
    first_instrument(&client).await?;
    Ok(())
}

async fn first_instrument(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let query = GetInstrumentsQuery { count: Some(1), ..Default::default() };
    let response = client.get_instruments(&query).await?;
    println!("received {} instruments", response.body.len());
    Ok(())
}
```

Build the client once and pass `&Client` to functions that make API calls. Authenticated calls use
caller-provided credentials in the same pattern:

```rust
use bitmex_client::{ApiCredentials, Client, Environment};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("BITMEX_TESTNET_API_KEY")?;
    let secret = std::env::var("BITMEX_TESTNET_API_SECRET")?;
    let credentials = ApiCredentials::new(key, secret)?;
    let client = Client::builder(Environment::Testnet)
        .credentials(credentials)
        .build()?;
    client.api_key_self().await?;
    Ok(())
}
```

`order::NewOrder` provides validated v2 limit, market, stop, and stop-limit constructors. `Client::place_order_v2` submits through `/api/v2/order`. The remaining documented v1 and v2 methods are available under their explicit generated names.

For market data, create a validated topic with
`realtime::Topic::new(realtime::Feed::OrderBookL2_25, Some(symbol), Some(realtime::Pool::Primary))`.
Connect using `Client::connect_realtime(realtime::Service::Primary)`, send `subscribe`, and drain
`next_event`. Treat `Event::Gap` as a request for a fresh image.

## Safety and realtime behavior

- API signatures cover the exact method, encoded path and query, expiry, and serialized JSON bytes. Credentials are injected and redacted in `Debug`.
- Financial JSON numbers and decimal strings are parsed as exact `rust_decimal::Decimal`; numeric JSON output uses the same exact decimal text.
- REST bodies and WebSocket frames are bounded. Cloned clients share rate admission, cooldown, and mutation fences.
- Mutations are single-attempt. An uncertain outcome fences the affected account or credential scope until the caller reconciles BitMEX state and calls `acknowledge_reconciliation`.
- Realtime connections never silently reconnect. A `Gap` means the caller must obtain a fresh image. The optional L2 book projection invalidates on a gap.

See [SECURITY.md](SECURITY.md) before using credentials or trading methods. No crates.io publication is part of this repository's initial change.

## Testing and maintenance

Normal CI is credential-free. It runs formatting, strict Clippy, all-feature tests, rustdoc,
offline generation and coverage checks, and `cargo package --locked` on Rust 1.95. Each of the
114 callable REST methods has deterministic loopback success and rejection fixtures. Unit tests
cover signing, exact decimals, shared mutation fences and rate admission, WebSocket lifecycle,
and L2 recovery. These tests establish the documented wire surface, not authenticated exchange
behavior.

The optional read-only Testnet probe is ignored and must be explicitly armed:

```sh
BITMEX_READ_ONLY_PROBE=I_ACCEPT_READ_ONLY_TESTNET \
cargo test --features live-tests --test testnet_read_only -- --ignored
```

The [CI workflow](.github/workflows/ci.yml) runs on PRs, pushes to `main`, a weekly schedule,
and manual dispatch. [Dependabot](.github/dependabot.yml) proposes weekly Cargo and GitHub
Actions updates. The manual [release-readiness workflow](.github/workflows/release-readiness.yml)
checks a requested version and reruns the local gates on `main`; it does not publish a crate or
create a GitHub release. See [CONTRIBUTING.md](CONTRIBUTING.md) for contract refresh and review
steps. Publication requires a separate reviewed change under [AGENTS.md](AGENTS.md).
