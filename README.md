# Deribit Rust Client

[![Crates.io](https://img.shields.io/crates/v/deribit-api.svg?style=flat-square&logo=rust)](https://crates.io/crates/deribit-api)
[![docs.rs](https://img.shields.io/docsrs/deribit-api?style=flat-square)](https://docs.rs/deribit-api)
[![CI](https://img.shields.io/github/actions/workflow/status/farhadi/deribit-api/ci.yml?branch=main&style=flat-square&logo=github)](https://github.com/farhadi/deribit-api/actions)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)

Type-safe, async Rust client for the Deribit WebSocket JSON‑RPC v2 API. Request/response types are generated at build time from the official API spec, and a single connection supports both RPC calls and streaming subscriptions.

## ✨ Features

- 🏗️ Build-time codegen from Deribit’s spec (production by default, optional Testnet)
- ⚡ Async WebSocket JSON‑RPC 2.0 over a single multiplexed connection
- 🦀 Strongly-typed requests, responses, channels and subscription notifications
- 📡 Simple subscriptions API for public and private channels
- 🔁 Concurrency-friendly: methods take `&self` (no `mut`), and the client is shareable via `Arc`
- 💓 Automatic heartbeat handling: responds to Deribit `test_request` internally (no manual pings needed)

## 🚀 Quick Start

Add the crate and `tokio` to your `Cargo.toml`:

```toml
[dependencies]
deribit-api = "0.1.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
futures-util = "0.3" # for StreamExt in subscription examples
```

### ▶️ Basic usage

Public call example: fetch server time.

```rust
use deribit_api::{DeribitClient, Env, PublicGetTimeRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DeribitClient::connect(Env::Production).await?;
    let time = client.call(PublicGetTimeRequest {}).await?;
    println!("Server time (µs): {:?}", time);
    Ok(())
}
```

### 🔐 Authentication + private methods

Authenticate using client credentials and fetch an account summary.

```rust
use deribit_api::{
    DeribitClient, Env, PublicAuthRequest, PublicAuthGrantType, PrivateGetAccountSummaryRequest,
    WalletCurrency,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DeribitClient::connect(Env::Production).await?;

    let client_id = std::env::var("DERIBIT_CLIENT_ID")?;
    let client_secret = std::env::var("DERIBIT_CLIENT_SECRET")?;

    let _auth = client
        .call(PublicAuthRequest {
            grant_type: PublicAuthGrantType::ClientCredentials,
            client_id,
            client_secret,
            ..Default::default()
        })
        .await?;

    let summary = client
        .call(PrivateGetAccountSummaryRequest {
            currency: WalletCurrency::Btc,
            ..Default::default()
        })
        .await?;
    println!("Account summary: {:?}", summary);
    Ok(())
}
```

### 📡 Streaming subscriptions

Untyped variant: subscribe by channel string and receive a Stream of `serde_json::Value`.

```rust
use deribit_api::{DeribitClient, Env};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DeribitClient::connect(Env::Production).await?;

    let mut stream = client.subscribe_raw("trades.BTC-PERPETUAL.raw").await?;

    while let Some(msg) = stream.next().await {
        println!("{:?}", msg);
    }
    Ok(())
}
```

Typed variant: use generated channel types and get a Stream of typed messages.

```rust
use deribit_api::{DeribitClient, Env, SubscriptionInterval, TradesInstrumentNameChannel};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DeribitClient::connect(Env::Production).await?;

    let channel = TradesInstrumentNameChannel {
        instrument_name: "BTC-PERPETUAL".to_string(),
        interval: SubscriptionInterval::Agg2,
    };
    let mut stream = client.subscribe(channel).await?;

    while let Some(msg) = stream.next().await {
        println!("{:?}", msg);
    }
    Ok(())
}
```

### 🧪 Testnet

- Connect with `Env::Testnet`:

```rust
let client = DeribitClient::connect(Env::Testnet).await?;
```

- Enable the feature to also generate Testnet types:

```toml
[dependencies]
deribit-api = { version = "0.1.1", features = ["testnet"] }
```

When the `testnet` feature is enabled, production types live at the crate root (`deribit_api::*`), and Testnet‑generated types are available under `deribit_api::testnet::*`.

Note: Enable the `testnet` feature only if you need endpoints or fields that exist only on Testnet. If you don't need any Testnet‑specific features, you can connect to `Env::Testnet` while using the default production spec and all overlapping APIs will work as expected.

## 🧩 API model

- Each endpoint like `public/get_time` maps to a request struct named `PublicGetTimeRequest`.
- Send requests via `client.call(request).await`.
- Responses deserialize into generated structs/enums where possible, or `serde_json::Value` for generic schemas.
- Subscriptions expose generated channel structs (e.g., `TradesInstrumentNameChannel`) implementing the `Subscription` trait. Use `client.subscribe(channel).await?` for typed streams, or `client.subscribe_raw("...")` for untyped.

Error type: all calls return `Result<T, deribit_api::Error>` (covers RPC, WebSocket, and JSON decode errors).

### 🧵 Low-level: `call_raw`

If you want to call a method by name with ad‑hoc JSON parameters, use `call_raw`. It returns a `serde_json::Value`.

Requires adding `serde_json` to your `Cargo.toml`:

```toml
[dependencies]
serde_json = "1"
```

```rust
use deribit_api::{DeribitClient, Env};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DeribitClient::connect(Env::Production).await?;

    // Public call
    let time = client.call_raw("public/get_time", json!({})).await?;
    println!("Server time (µs): {}", time);

    // Authenticate (enables private methods on this connection)
    let _auth = client
        .call_raw(
            "public/auth",
            json!({
                "grant_type": "client_credentials",
                "client_id": std::env::var("DERIBIT_CLIENT_ID")?,
                "client_secret": std::env::var("DERIBIT_CLIENT_SECRET")?,
            }),
        )
        .await?;

    // Private call
    let account = client
        .call_raw(
            "private/get_account_summary",
            json!({ "currency": "BTC" }),
        )
        .await?;
    println!("Account summary: {}", account);

    Ok(())
}
```

### 🤝 Concurrency and sharing

The client is safe to share across tasks using `std::sync::Arc` and does not require `mut`. All methods take `&self` and internally multiplex over a single WebSocket connection.

```rust
use std::sync::Arc;
use deribit_api::{DeribitClient, Env, PublicGetTimeRequest, SubscriptionInterval, TradesInstrumentNameChannel};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Arc::new(DeribitClient::connect(Env::Production).await?);

    // Task 1: make RPC calls
    let c1 = client.clone();
    let h1 = tokio::spawn(async move {
        let _ = c1.call(PublicGetTimeRequest {}).await;
    });

    // Task 2: subscribe to a channel
    let c2 = client.clone();
    let h2 = tokio::spawn(async move {
        let mut stream = c2.subscribe(TradesInstrumentNameChannel {
            instrument_name: "BTC-PERPETUAL".to_string(),
            interval: SubscriptionInterval::Agg2,
        }).await.unwrap();
        while let Some(_data) = stream.next().await {}
    });

    let _ = tokio::join!(h1, h2);
    Ok(())
}
```

## 🔧 Configuration

- Default spec source: production `https://www.deribit.com/static/deribit_api_v2.json`.
- Override the API spec used for codegen at build time in one of these ways:
  - Enable the `bundled-spec` feature to force using bundled `deribit_api_v2.json` file:
    - Enabling `bundled-spec` feature in `Cargo.toml`:
      ```toml
      [dependencies]
      deribit-api = { version = "0.1.1", features = ["bundled-spec"] }
      ```
    - Running tests using bundled spec:
      `cargo test --features bundled-spec`
  - Environment variable `DERIBIT_API_SPEC` pointing to a local file path or a URL.
    - Examples:
      - `DERIBIT_API_SPEC=./deribit_api_v2.json cargo build`
      - `DERIBIT_API_SPEC=https://example.com/deribit_api_v2.json cargo build`

- Testnet codegen: enable `testnet` to also generate Testnet types alongside production:
   - Enabling `testnet` feature in `Cargo.toml`:
      ```toml
      [dependencies]
      deribit-api = { version = "0.1.1", features = ["testnet"] }
      ```
  - Production types are at the crate root (`deribit_api::*`); Testnet types live under `deribit_api::testnet::*`.
  - Only enable this if you need new Testnet endpoints/fields that are not available on production; otherwise you can use `Env::Testnet` with the default production spec.

- The build script also sets `GENERATED_DERIBIT_CLIENT_PATH` (env var) to the formatted, generated production client file path in `target/`, which can help with debugging.

## 📚 Examples

This repo ships several runnable examples:

```bash
# Public calls
cargo run --example basic_usage

# Subscriptions
cargo run --example subscription

# Authentication + private endpoints
cargo run --example authentication

# Setting heartbeats
cargo run --example heartbeat

# Testnet (enables the feature and uses the Testnet endpoint)
cargo run --features testnet --example testnet

# Low-level + untyped stream
cargo run --example untyped

# Concurrent RPC + subscription on one connection
cargo run --example concurrent
```

## 🛠️ Development

```bash
cargo build
cargo check --examples
cargo test
```

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

## ⚠️ Disclaimer

This software is for educational and development purposes. Use at your own risk when trading with real funds.
