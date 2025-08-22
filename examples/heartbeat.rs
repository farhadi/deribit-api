use deribit_api::{
    DeribitClient, DeribitPriceIndexIndexNameChannel, Env, IndexName, PublicSetHeartbeatRequest,
};
use futures_util::stream::{StreamExt, select};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DeribitClient::connect(Env::Production).await?;

    client
        .call(PublicSetHeartbeatRequest { interval: 10 })
        .await?;

    let btc_stream = client
        .subscribe(DeribitPriceIndexIndexNameChannel {
            index_name: IndexName::BtcUsd,
        })
        .await?;

    let eth_stream = client
        .subscribe(DeribitPriceIndexIndexNameChannel {
            index_name: IndexName::EthUsd,
        })
        .await?;

    let mut stream = select(btc_stream, eth_stream);

    while let Some(Ok(price)) = stream.next().await {
        println!("Price: {:?}", price);
    }

    Ok(())
}
