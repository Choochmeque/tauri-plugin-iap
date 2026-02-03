use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProductsRequest {
    pub product_ids: Vec<String>,
    #[serde(default = "default_product_type")]
    pub product_type: String,
}

fn default_product_type() -> String {
    "subs".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingPhase {
    pub formatted_price: String,
    pub price_currency_code: String,
    pub price_amount_micros: i64,
    pub billing_period: String,
    pub billing_cycle_count: i32,
    pub recurrence_mode: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionOffer {
    pub offer_token: String,
    pub base_plan_id: String,
    pub offer_id: Option<String>,
    pub pricing_phases: Vec<PricingPhase>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    pub product_id: String,
    pub title: String,
    pub description: String,
    pub product_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatted_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_currency_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_amount_micros: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_offer_details: Option<Vec<SubscriptionOffer>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProductsResponse {
    pub products: Vec<Product>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PurchaseOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfuscated_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfuscated_profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_account_token: Option<String>,
    /// Purchase token of the existing subscription to replace (Android only).
    /// When set, the purchase becomes a subscription upgrade/downgrade.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_purchase_token: Option<String>,
    /// Replacement mode for subscription upgrades/downgrades (Android only).
    /// Maps to Google Play BillingFlowParams.SubscriptionUpdateParams.ReplacementMode.
    /// Defaults to WITH_TIME_PRORATION (1) if not specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_replacement_mode: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseRequest {
    pub product_id: String,
    #[serde(default = "default_product_type")]
    pub product_type: String,
    #[serde(flatten)]
    pub options: Option<PurchaseOptions>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Purchase {
    pub order_id: Option<String>,
    pub package_name: String,
    pub product_id: String,
    pub purchase_time: i64,
    pub purchase_token: String,
    pub purchase_state: PurchaseStateValue,
    pub is_auto_renewing: bool,
    pub is_acknowledged: bool,
    pub original_json: String,
    pub signature: String,
    pub original_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jws_representation: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePurchasesRequest {
    #[serde(default = "default_product_type")]
    pub product_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePurchasesResponse {
    pub purchases: Vec<Purchase>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseHistoryRecord {
    pub product_id: String,
    pub purchase_time: i64,
    pub purchase_token: String,
    pub quantity: i32,
    pub original_json: String,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPurchaseHistoryResponse {
    pub history: Vec<PurchaseHistoryRecord>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcknowledgePurchaseRequest {
    pub purchase_token: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcknowledgePurchaseResponse {
    pub success: bool,
}

/// Keep in sync with PurchaseState in guest-js/index.ts
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PurchaseStateValue {
    Purchased = 0,
    Canceled = 1,
    Pending = 2,
}

impl Serialize for PurchaseStateValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(*self as i32)
    }
}

impl<'de> Deserialize<'de> for PurchaseStateValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = i32::deserialize(deserializer)?;
        match value {
            0 => Ok(PurchaseStateValue::Purchased),
            1 => Ok(PurchaseStateValue::Canceled),
            2 => Ok(PurchaseStateValue::Pending),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid purchase state: {value}"
            ))),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProductStatusRequest {
    pub product_id: String,
    #[serde(default = "default_product_type")]
    pub product_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductStatus {
    pub product_id: String,
    pub is_owned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_state: Option<PurchaseStateValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_auto_renewing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_acknowledged: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_product_type() {
        assert_eq!(default_product_type(), "subs");
    }

    #[test]
    fn test_purchase_state_value_serialize() {
        assert_eq!(
            serde_json::to_string(&PurchaseStateValue::Purchased)
                .expect("Failed to serialize Purchased state"),
            "0"
        );
        assert_eq!(
            serde_json::to_string(&PurchaseStateValue::Canceled)
                .expect("Failed to serialize Canceled state"),
            "1"
        );
        assert_eq!(
            serde_json::to_string(&PurchaseStateValue::Pending)
                .expect("Failed to serialize Pending state"),
            "2"
        );
    }

    #[test]
    fn test_purchase_state_value_deserialize() {
        assert_eq!(
            serde_json::from_str::<PurchaseStateValue>("0")
                .expect("Failed to deserialize Purchased state"),
            PurchaseStateValue::Purchased
        );
        assert_eq!(
            serde_json::from_str::<PurchaseStateValue>("1")
                .expect("Failed to deserialize Canceled state"),
            PurchaseStateValue::Canceled
        );
        assert_eq!(
            serde_json::from_str::<PurchaseStateValue>("2")
                .expect("Failed to deserialize Pending state"),
            PurchaseStateValue::Pending
        );
    }

    #[test]
    fn test_purchase_state_value_deserialize_invalid() {
        let result = serde_json::from_str::<PurchaseStateValue>("3");
        assert!(result.is_err());
        let err = result
            .expect_err("Expected error for invalid state")
            .to_string();
        assert!(err.contains("Invalid purchase state: 3"));
    }

    #[test]
    fn test_purchase_state_value_roundtrip() {
        for state in [
            PurchaseStateValue::Purchased,
            PurchaseStateValue::Canceled,
            PurchaseStateValue::Pending,
        ] {
            let serialized =
                serde_json::to_string(&state).expect("Failed to serialize PurchaseStateValue");
            let deserialized: PurchaseStateValue = serde_json::from_str(&serialized)
                .expect("Failed to deserialize PurchaseStateValue");
            assert_eq!(state, deserialized);
        }
    }

    #[test]
    fn test_initialize_response_default() {
        let response = InitializeResponse::default();
        assert!(!response.success);
    }

    #[test]
    fn test_initialize_response_serde() {
        let response = InitializeResponse { success: true };
        let json =
            serde_json::to_string(&response).expect("Failed to serialize InitializeResponse");
        assert_eq!(json, r#"{"success":true}"#);

        let deserialized: InitializeResponse =
            serde_json::from_str(&json).expect("Failed to deserialize InitializeResponse");
        assert!(deserialized.success);
    }

    #[test]
    fn test_get_products_request_default_product_type() {
        let json = r#"{"productIds":["product1","product2"]}"#;
        let request: GetProductsRequest =
            serde_json::from_str(json).expect("Failed to deserialize GetProductsRequest");
        assert_eq!(request.product_ids, vec!["product1", "product2"]);
        assert_eq!(request.product_type, "subs");
    }

    #[test]
    fn test_get_products_request_explicit_product_type() {
        let json = r#"{"productIds":["product1"],"productType":"inapp"}"#;
        let request: GetProductsRequest =
            serde_json::from_str(json).expect("Failed to deserialize GetProductsRequest");
        assert_eq!(request.product_type, "inapp");
    }

    #[test]
    fn test_product_optional_fields_skip_serializing() {
        let product = Product {
            product_id: "test".to_string(),
            title: "Test Product".to_string(),
            description: "A test product".to_string(),
            product_type: "inapp".to_string(),
            formatted_price: None,
            price_currency_code: None,
            price_amount_micros: None,
            subscription_offer_details: None,
        };
        let json = serde_json::to_string(&product).expect("Failed to serialize Product");
        assert!(!json.contains("formattedPrice"));
        assert!(!json.contains("priceCurrencyCode"));
        assert!(!json.contains("priceAmountMicros"));
        assert!(!json.contains("subscriptionOfferDetails"));
    }

    #[test]
    fn test_product_with_optional_fields() {
        let product = Product {
            product_id: "test".to_string(),
            title: "Test Product".to_string(),
            description: "A test product".to_string(),
            product_type: "inapp".to_string(),
            formatted_price: Some("$9.99".to_string()),
            price_currency_code: Some("USD".to_string()),
            price_amount_micros: Some(9990000),
            subscription_offer_details: None,
        };
        let json = serde_json::to_string(&product).expect("Failed to serialize Product");
        assert!(json.contains(r#""formattedPrice":"$9.99""#));
        assert!(json.contains(r#""priceCurrencyCode":"USD""#));
        assert!(json.contains(r#""priceAmountMicros":9990000"#));
    }

    #[test]
    fn test_purchase_serde_roundtrip() {
        let purchase = Purchase {
            order_id: Some("order123".to_string()),
            package_name: "com.example.app".to_string(),
            product_id: "product1".to_string(),
            purchase_time: 1700000000000,
            purchase_token: "token123".to_string(),
            purchase_state: PurchaseStateValue::Purchased,
            is_auto_renewing: true,
            is_acknowledged: false,
            original_json: "{}".to_string(),
            signature: "sig".to_string(),
            original_id: None,
            jws_representation: Some("test_jws".to_string()),
        };

        let json = serde_json::to_string(&purchase).expect("Failed to serialize Purchase");
        let deserialized: Purchase =
            serde_json::from_str(&json).expect("Failed to deserialize Purchase");

        assert_eq!(deserialized.order_id, purchase.order_id);
        assert_eq!(deserialized.product_id, purchase.product_id);
        assert_eq!(deserialized.purchase_time, purchase.purchase_time);
        assert_eq!(deserialized.purchase_state, purchase.purchase_state);
        assert_eq!(deserialized.is_auto_renewing, purchase.is_auto_renewing);
    }

    #[test]
    fn test_pricing_phase_serde() {
        let phase = PricingPhase {
            formatted_price: "$4.99".to_string(),
            price_currency_code: "USD".to_string(),
            price_amount_micros: 4990000,
            billing_period: "P1M".to_string(),
            billing_cycle_count: 1,
            recurrence_mode: 1,
        };

        let json = serde_json::to_string(&phase).expect("Failed to serialize PricingPhase");
        assert!(json.contains(r#""formattedPrice":"$4.99""#));
        assert!(json.contains(r#""billingPeriod":"P1M""#));

        let deserialized: PricingPhase =
            serde_json::from_str(&json).expect("Failed to deserialize PricingPhase");
        assert_eq!(deserialized.price_amount_micros, 4990000);
    }

    #[test]
    fn test_subscription_offer_serde() {
        let offer = SubscriptionOffer {
            offer_token: "token123".to_string(),
            base_plan_id: "base_plan".to_string(),
            offer_id: Some("offer1".to_string()),
            pricing_phases: vec![PricingPhase {
                formatted_price: "$9.99".to_string(),
                price_currency_code: "USD".to_string(),
                price_amount_micros: 9990000,
                billing_period: "P1M".to_string(),
                billing_cycle_count: 0,
                recurrence_mode: 1,
            }],
        };

        let json = serde_json::to_string(&offer).expect("Failed to serialize SubscriptionOffer");
        let deserialized: SubscriptionOffer =
            serde_json::from_str(&json).expect("Failed to deserialize SubscriptionOffer");
        assert_eq!(deserialized.offer_token, "token123");
        assert_eq!(deserialized.pricing_phases.len(), 1);
    }

    #[test]
    fn test_purchase_options_flatten() {
        let json = r#"{"productId":"prod1","offerToken":"token","obfuscatedAccountId":"acc123"}"#;
        let request: PurchaseRequest =
            serde_json::from_str(json).expect("Failed to deserialize PurchaseRequest");

        assert_eq!(request.product_id, "prod1");
        assert_eq!(request.product_type, "subs"); // default
        let opts = request
            .options
            .expect("Expected PurchaseOptions to be present");
        assert_eq!(opts.offer_token, Some("token".to_string()));
        assert_eq!(opts.obfuscated_account_id, Some("acc123".to_string()));
        assert_eq!(opts.old_purchase_token, None);
        assert_eq!(opts.subscription_replacement_mode, None);
    }

    #[test]
    fn test_purchase_options_with_subscription_update() {
        let json = r#"{"productId":"prod1","offerToken":"token","oldPurchaseToken":"old_token_123","subscriptionReplacementMode":2}"#;
        let request: PurchaseRequest =
            serde_json::from_str(json).expect("Failed to deserialize PurchaseRequest");

        assert_eq!(request.product_id, "prod1");
        let opts = request
            .options
            .expect("Expected PurchaseOptions to be present");
        assert_eq!(opts.offer_token, Some("token".to_string()));
        assert_eq!(opts.old_purchase_token, Some("old_token_123".to_string()));
        assert_eq!(opts.subscription_replacement_mode, Some(2));
    }

    #[test]
    fn test_restore_purchases_request_default() {
        let json = r#"{}"#;
        let request: RestorePurchasesRequest =
            serde_json::from_str(json).expect("Failed to deserialize RestorePurchasesRequest");
        assert_eq!(request.product_type, "subs");
    }

    #[test]
    fn test_product_status_optional_fields() {
        let status = ProductStatus {
            product_id: "prod1".to_string(),
            is_owned: false,
            purchase_state: None,
            purchase_time: None,
            expiration_time: None,
            is_auto_renewing: None,
            is_acknowledged: None,
            purchase_token: None,
        };

        let json = serde_json::to_string(&status).expect("Failed to serialize ProductStatus");
        // Optional None fields should be skipped
        assert!(!json.contains("purchaseState"));
        assert!(!json.contains("purchaseTime"));
        assert!(!json.contains("expirationTime"));
    }

    #[test]
    fn test_product_status_with_values() {
        let status = ProductStatus {
            product_id: "prod1".to_string(),
            is_owned: true,
            purchase_state: Some(PurchaseStateValue::Purchased),
            purchase_time: Some(1700000000000),
            expiration_time: Some(1703000000000),
            is_auto_renewing: Some(true),
            is_acknowledged: Some(true),
            purchase_token: Some("token123".to_string()),
        };

        let json = serde_json::to_string(&status).expect("Failed to serialize ProductStatus");
        assert!(json.contains(r#""isOwned":true"#));
        assert!(json.contains(r#""purchaseState":0"#));
        assert!(json.contains(r#""isAutoRenewing":true"#));
    }

    #[test]
    fn test_acknowledge_purchase_request_serde() {
        let request = AcknowledgePurchaseRequest {
            purchase_token: "token123".to_string(),
        };
        let json = serde_json::to_string(&request)
            .expect("Failed to serialize AcknowledgePurchaseRequest");
        assert_eq!(json, r#"{"purchaseToken":"token123"}"#);
    }

    #[test]
    fn test_get_product_status_request_serde() {
        let json = r#"{"productId":"prod1"}"#;
        let request: GetProductStatusRequest =
            serde_json::from_str(json).expect("Failed to deserialize GetProductStatusRequest");
        assert_eq!(request.product_id, "prod1");
        assert_eq!(request.product_type, "subs"); // default
    }

    #[test]
    fn test_purchase_history_record_serde() {
        let record = PurchaseHistoryRecord {
            product_id: "prod1".to_string(),
            purchase_time: 1700000000000,
            purchase_token: "token".to_string(),
            quantity: 1,
            original_json: "{}".to_string(),
            signature: "sig".to_string(),
        };

        let json =
            serde_json::to_string(&record).expect("Failed to serialize PurchaseHistoryRecord");
        let deserialized: PurchaseHistoryRecord =
            serde_json::from_str(&json).expect("Failed to deserialize PurchaseHistoryRecord");
        assert_eq!(deserialized.quantity, 1);
    }
}
