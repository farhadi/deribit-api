use deribit_api::{
    CurrencyWithAny, DeribitClient, Env, Kind, PublicGetCurrenciesRequest,
    PublicGetInstrumentsRequest, PublicGetOrderBookRequest, PublicGetTimeRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to Testnet to avoid hitting production while experimenting
    let client = DeribitClient::connect(Env::Production).await?;

    // 1) Get server time
    let time = client.call(PublicGetTimeRequest {}).await?;
    println!("Server time (µs): {:?}", time);

    // 2) Get supported currencies
    let currencies = client.call(PublicGetCurrenciesRequest {}).await?;
    println!("Currencies ({}): {:?}", currencies.len(), currencies);

    // 3) List instruments (example: BTC futures, not expired)
    let instruments = client
        .call(PublicGetInstrumentsRequest {
            currency: CurrencyWithAny::Btc,
            kind: Some(Kind::Future),
            expired: Some(false),
        })
        .await?;
    println!("BTC futures instruments: {:?}", instruments);

    // 4) Fetch an order book snapshot (example: BTC perpetual)
    let order_book = client
        .call(PublicGetOrderBookRequest {
            instrument_name: "BTC-PERPETUAL".to_string(),
            depth: Some(5),
        })
        .await?;
    println!("Order book (top 5): {:?}", order_book);

    Ok(())
}
