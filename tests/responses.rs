use deribit_api::*;
use serde_json::json;

#[test]
fn public_get_time_response_deserializes_from_number() {
    type Resp = <PublicGetTimeRequest as ApiRequest>::Response;
    let raw = json!(1_755_765_833_825i64);
    let resp: Resp = serde_json::from_value(raw).expect("response type should accept a number");
    assert_eq!(resp, 1_755_765_833_825i64);
}

#[test]
fn public_get_currencies_response_deserializes() {
    type Resp = <PublicGetCurrenciesRequest as ApiRequest>::Response;
    let raw = json!([{
      "currency": "ETH",
      "apr": 0,
      "min_withdrawal_fee": 0.001,
      "withdrawal_fee": 0.001,
      "fee_precision": 4,
      "coin_type": "ETH",
      "withdrawal_priorities": [],
      "min_confirmations": 50,
      "currency_long": "Ethereum",
      "in_cross_collateral_pool": true
    },
    {
      "currency": "BTC",
      "apr": 0,
      "min_withdrawal_fee": 0.00001,
      "withdrawal_fee": 0.00001,
      "fee_precision": 5,
      "coin_type": "BTC",
      "withdrawal_priorities": [],
      "min_confirmations": 1,
      "currency_long": "Bitcoin",
      "in_cross_collateral_pool": true
    }]);
    let resp: Resp =
        serde_json::from_value(raw).expect("JSON response should deserialize to typed response");
    assert_eq!(
        resp,
        vec![
            CurrencyWithApr {
                currency: "ETH".to_string(),
                apr: Some(0.0),
                min_withdrawal_fee: Some(0.001),
                withdrawal_fee: 0.001,
                fee_precision: Some(4),
                coin_type: CurrencyWithAprCoinType::Eth,
                withdrawal_priorities: Some(vec![]),
                min_confirmations: Some(50),
                currency_long: "Ethereum".to_string(),
                in_cross_collateral_pool: true,
            },
            CurrencyWithApr {
                currency: "BTC".to_string(),
                apr: Some(0.0),
                min_withdrawal_fee: Some(0.00001),
                withdrawal_fee: 0.00001,
                fee_precision: Some(5),
                coin_type: CurrencyWithAprCoinType::Btc,
                withdrawal_priorities: Some(vec![]),
                min_confirmations: Some(1),
                currency_long: "Bitcoin".to_string(),
                in_cross_collateral_pool: true,
            },
        ]
    );
}
