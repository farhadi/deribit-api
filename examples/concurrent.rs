use deribit_api::{
    DeribitClient, Env, PublicGetTimeRequest, SubscriptionInterval, TradesInstrumentNameChannel,
};
use futures_util::StreamExt;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Arc::new(DeribitClient::connect(Env::Production).await?);

    // Task 1: make RPC calls
    let c1 = client.clone();
    let h1 = tokio::spawn(async move {
        let time = c1.call(PublicGetTimeRequest {}).await.unwrap();
        println!("Time: {:?}", time);
    });

    // Task 2: subscribe to a channel
    let c2 = client.clone();
    let h2 = tokio::spawn(async move {
        let mut stream = c2
            .subscribe(TradesInstrumentNameChannel {
                instrument_name: "BTC-PERPETUAL".to_string(),
                interval: SubscriptionInterval::Agg2,
            })
            .await
            .unwrap();
        while let Some(Ok(data)) = stream.next().await {
            println!("{:?}", data);
        }
    });

    let _ = tokio::join!(h1, h2);
    Ok(())
}
