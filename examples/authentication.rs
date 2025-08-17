use deribit_api::{
    CurrencyWithAny, DeribitClient, Env, PrivateGetAccountSummaryRequest,
    PrivateGetOpenOrdersRequest, PrivateGetPositionsRequest, PublicAuthGrantType,
    PublicAuthRequest, WalletCurrency,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to Testnet for safety
    let client = DeribitClient::connect(Env::Production).await?;

    // Read credentials from env vars
    let client_id =
        std::env::var("DERIBIT_CLIENT_ID").expect("DERIBIT_CLIENT_ID is required for this example");
    let client_secret = std::env::var("DERIBIT_CLIENT_SECRET")
        .expect("DERIBIT_CLIENT_SECRET is required for this example");

    // Authenticate using client credentials
    let auth = client
        .call(PublicAuthRequest {
            grant_type: PublicAuthGrantType::ClientCredentials,
            client_id,
            client_secret,
            ..Default::default()
        })
        .await?;
    println!(
        "Authenticated. Access token starts with: {}...",
        &auth.access_token.chars().take(6).collect::<String>()
    );

    // Now we can call private endpoints. Example: account summary (BTC wallet)
    let account = client
        .call(PrivateGetAccountSummaryRequest {
            currency: WalletCurrency::Btc,
            ..Default::default()
        })
        .await?;
    println!("Account summary: {:?}", account);

    let positions = client
        .call(PrivateGetPositionsRequest {
            currency: Some(CurrencyWithAny::Btc),
            ..Default::default()
        })
        .await?;
    println!("Positions: {:?}", positions);

    let orders = client
        .call(PrivateGetOpenOrdersRequest {
            ..Default::default()
        })
        .await?;
    println!("Orders: {:?}", orders);

    Ok(())
}
