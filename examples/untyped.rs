use deribit_api::{DeribitClient, Env};
use futures_util::StreamExt;
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
        .call_raw("private/get_account_summary", json!({ "currency": "BTC" }))
        .await?;
    println!("Account summary: {}", account);

    // Untyped subscription
    let mut stream = client.subscribe_raw("trades.BTC-PERPETUAL.raw").await?;

    while let Some(Ok(msg)) = stream.next().await {
        println!("{:?}", msg);
    }

    Ok(())
}
