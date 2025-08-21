use deribit_api::*;

#[test]
fn trades_channel_string_matches_pattern() {
    // Channel string looks like: trades.{instrument_name}.{interval}
    let ch = TradesInstrumentNameChannel {
        instrument_name: "BTC-PERPETUAL".to_string(),
        interval: SubscriptionInterval::Agg2,
    };
    let channel_str = ch.channel_string();
    assert_eq!(channel_str, "trades.BTC-PERPETUAL.agg2");
}

#[test]
fn book_channel_string_matches_pattern() {
    // Channel string looks like: book.{instrument_name}.{group}.{depth}.{interval}
    let ch = BookInstrumentNameGroupDepthChannel {
        instrument_name: "BTC-PERPETUAL".to_string(),
        group: BookInstrumentNameGroupDepthGroup::None,
        depth: 10,
        interval: BookInstrumentNameGroupDepthInterval::Agg2,
    };
    let channel_str = ch.channel_string();
    assert_eq!(channel_str, "book.BTC-PERPETUAL.none.10.agg2");
}
