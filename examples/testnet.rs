#[cfg(not(feature = "testnet"))]
fn main() {
    eprintln!(
        "This example requires the 'testnet' feature. Run with:\n  cargo run --features testnet --example testnet"
    );
}

#[cfg(feature = "testnet")]
use deribit_api::{DeribitClient, Env};

#[cfg(feature = "testnet")]
use deribit_api::{PublicGetCurrenciesRequest, PublicGetTimeRequest};

#[cfg(feature = "testnet")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to Deribit Testnet
    let client = DeribitClient::connect(Env::Testnet).await?;

    // 1) Get server time from Testnet
    let time = client.call(PublicGetTimeRequest {}).await?;
    println!("Testnet server time (µs): {:?}", time);

    // 2) Get supported currencies on Testnet
    let currencies = client.call(PublicGetCurrenciesRequest {}).await?;
    println!(
        "Testnet currencies ({}): {:?}",
        currencies.len(),
        currencies
    );

    Ok(())
}
