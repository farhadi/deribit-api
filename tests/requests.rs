use deribit_api::*;
use serde_json::{Value, json};

#[test]
fn public_get_time_request_serializes_to_empty_object() {
    let req = PublicGetTimeRequest::default();
    let params = req.to_params();
    assert_eq!(params, json!({}));
    assert_eq!(req.method_name(), "public/get_time");
    assert!(!req.is_private());
}

#[test]
fn private_get_account_summary_is_private_and_has_required_param() {
    // Required field `currency` should be present (no Option)
    let req = PrivateGetAccountSummaryRequest {
        currency: WalletCurrency::Btc,
        ..Default::default()
    };
    assert_eq!(req.method_name(), "private/get_account_summary");
    assert!(req.is_private());

    // Serialization should include the required field
    let params = req.to_params();
    assert_eq!(
        params.get("currency"),
        Some(&Value::String("BTC".to_string()))
    );
}

#[test]
fn public_auth_request_serialization_skips_nones() {
    let req = PublicAuthRequest {
        grant_type: PublicAuthGrantType::ClientCredentials,
        client_id: "id".into(),
        client_secret: "secret".into(),
        ..Default::default()
    };
    let params = req.to_params();
    // Required fields present
    assert_eq!(
        params.get("grant_type"),
        Some(&Value::String("client_credentials".into()))
    );
    assert_eq!(params.get("client_id"), Some(&Value::String("id".into())));
    assert_eq!(
        params.get("client_secret"),
        Some(&Value::String("secret".into()))
    );
    // Optional fields omitted when None
    assert!(params.get("nonce").is_none());
    assert!(params.get("state").is_none());
}
