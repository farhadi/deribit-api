use deribit_api::*;
use serde_json::Value;

#[test]
fn wallet_currency_enum_serializes_as_expected() {
    // Ensure enums from the spec exist and serialize to their string values
    let btc = serde_json::to_value(WalletCurrency::Btc).unwrap();
    assert_eq!(btc, Value::String("BTC".to_string()));
}

#[test]
fn subscription_interval_serializes() {
    let val = serde_json::to_value(SubscriptionInterval::Agg2).unwrap();
    assert_eq!(val, Value::String("agg2".into()));
}

#[test]
fn public_auth_grant_type_serializes() {
    let val = serde_json::to_value(PublicAuthGrantType::ClientCredentials).unwrap();
    assert_eq!(val, Value::String("client_credentials".into()));
}
