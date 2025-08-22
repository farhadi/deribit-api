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

    while let Some(Ok(msg)) = stream.next().await {
        println!("{:?}", msg);
    }

    Ok(())
}
