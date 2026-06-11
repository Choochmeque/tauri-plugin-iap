use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use nt_time::FileTime;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri::Manager;
use tauri::{AppHandle, Runtime, plugin::PluginApi};
use windows::core::{HSTRING, Interface};
use windows::{
    Foundation::DateTime,
    Services::Store::{
        StoreConsumableStatus, StoreContext, StoreLicense, StoreProduct, StorePurchaseProperties,
        StorePurchaseStatus,
    },
    Win32::UI::Shell::IInitializeWithWindow,
};
use windows_collections::IIterable;

use crate::error::{ErrorResponse, PluginInvokeError};
use crate::models::{
    GetProductsResponse, PricingPhase, Product, ProductStatus, Purchase, PurchaseRequest,
    PurchaseStateValue, RestorePurchasesRequest, RestorePurchasesResponse, SubscriptionOffer,
};
use std::sync::{Arc, RwLock};

fn reject(code: &str, message: impl Into<String>) -> crate::Error {
    crate::Error::PluginInvoke(PluginInvokeError::InvokeRejected(ErrorResponse {
        code: Some(code.to_string()),
        message: Some(message.into()),
        data: (),
    }))
}

/// Microsoft Store has no native per-transaction token, but our cross-platform API
/// exposes `purchase_token: String` for every purchase. We synthesize one by encoding
/// the data needed to consume the purchase later (the Microsoft `StoreId`, required
/// by `ReportConsumableFulfillmentAsync`) into a versioned base64-JSON envelope.
/// The token is opaque to the developer; only `consume_purchase()` decodes it.
///
/// `tracking_id` is a fresh GUID per token, giving every emitted purchase a unique
/// identifier even when two are generated within the same millisecond.
#[derive(Serialize, Deserialize)]
struct WindowsPurchaseTokenV1 {
    v: u8,
    store_id: String,
    purchase_time: i64,
    tracking_id: String,
}

impl WindowsPurchaseTokenV1 {
    fn new(store_id: String, purchase_time: i64) -> crate::Result<Self> {
        let tracking_id = format!("{:032x}", windows::core::GUID::new()?.to_u128());
        Ok(Self {
            v: 1,
            store_id,
            purchase_time,
            tracking_id,
        })
    }

    fn encode(&self) -> crate::Result<String> {
        let bytes = serde_json::to_vec(self).map_err(|e| {
            reject(
                "internalError",
                format!("Failed to encode purchase token: {e}"),
            )
        })?;
        Ok(URL_SAFE_NO_PAD.encode(&bytes))
    }

    fn decode(s: &str) -> crate::Result<Self> {
        let invalid = || reject("invalidPurchaseToken", "Invalid Windows purchase token");
        let bytes = URL_SAFE_NO_PAD.decode(s).map_err(|_| invalid())?;
        let env: Self = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if env.v != 1 || env.store_id.trim().is_empty() || env.tracking_id.trim().is_empty() {
            return Err(invalid());
        }
        Ok(env)
    }
}

#[allow(clippy::unnecessary_wraps)]
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: &PluginApi<R, C>,
) -> crate::Result<Iap<R>> {
    Ok(Iap {
        app_handle: app.clone(),
        store_context: Arc::new(RwLock::new(None)),
    })
}

/// Access to the iap APIs.
pub struct Iap<R: Runtime> {
    app_handle: AppHandle<R>,
    store_context: Arc<RwLock<Option<StoreContext>>>,
}

impl<R: Runtime> Iap<R> {
    /// Get or create the `StoreContext` instance
    fn get_store_context(&self) -> crate::Result<StoreContext> {
        let mut context_guard = self.store_context.write().map_err(|e| {
            reject(
                "internalError",
                format!("Failed to acquire write lock: {e:?}"),
            )
        })?;

        if context_guard.is_none() {
            // Get the default store context for the current user
            let context = StoreContext::GetDefault()?;

            let window = self
                .app_handle
                .get_webview_window("main")
                .ok_or_else(|| reject("windowError", "Failed to get main window"))?;
            let hwnd = window.hwnd().map_err(|e| {
                reject("windowError", format!("Failed to get window handle: {e:?}"))
            })?;

            // Cast the WinRT object to IInitializeWithWindow and initialize it with your HWND
            let init = context.cast::<IInitializeWithWindow>()?;
            unsafe {
                init.Initialize(hwnd)?;
            }

            *context_guard = Some(context);
        }

        Ok(context_guard
            .as_ref()
            .ok_or_else(|| reject("storeNotInitialized", "Store context not initialized"))?
            .clone())
    }

    /// Convert Windows `DateTime` to Unix timestamp in milliseconds.
    ///
    /// `Foundation::DateTime::UniversalTime` and Windows `FILETIME` share the
    /// same representation (100-nanosecond ticks since 1601-01-01 UTC).
    #[allow(clippy::cast_sign_loss)]
    fn datetime_to_unix_millis(datetime: DateTime) -> i64 {
        FileTime::new(datetime.UniversalTime as u64).to_unix_time_millis()
    }

    /// Emit an event to the frontend (equivalent to `iOS`/Android `trigger` method).
    fn trigger<S: serde::Serialize + Clone>(&self, event: &str, payload: S) {
        let _ = self.app_handle.emit(event, payload);
    }

    /// Mint a Microsoft Store ID key (JWT) bound to the current
    /// Microsoft account signed into the device. Backends use the key
    /// as `b2bKey` / `beneficiaries[].identityValue` when calling the
    /// Microsoft Store services to query the user's purchases without
    /// holding the user's MSA.
    ///
    /// Microsoft mints **per-API** keys: the one returned by
    /// `GetCustomerPurchaseIdAsync` is only valid against
    /// `purchase.mp.microsoft.com` (used for subscription
    /// recurrence queries); the one from
    /// `GetCustomerCollectionsIdAsync` is only valid against
    /// `collections.mp.microsoft.com` (used for one-time product
    /// ownership queries). The two are not interchangeable —
    /// presenting one to the wrong surface yields
    /// `AuthenticationTokenInvalid` on the "B2B key". The plugin
    /// picks the right mint method from the existing
    /// `product_type` field already on every purchase / restore
    /// payload:
    ///   * `"subs"`  → `GetCustomerPurchaseIdAsync`
    ///   * `"inapp"` → `GetCustomerCollectionsIdAsync`
    ///
    /// `service_ticket` is an Entra ID access token with audience
    /// `https://onestore.microsoft.com`. `publisher_user_id` is
    /// embedded verbatim in the key as the `userId` claim so the
    /// backend can identity-bind the purchase.
    fn mint_store_id_key(
        &self,
        product_type: &str,
        service_ticket: &str,
        publisher_user_id: &str,
    ) -> crate::Result<String> {
        let context = self.get_store_context()?;
        let ticket = HSTRING::from(service_ticket);
        let user_id = HSTRING::from(publisher_user_id);
        let key = if product_type == "subs" {
            context
                .GetCustomerPurchaseIdAsync(&ticket, &user_id)
                .and_then(|op| op.get())?
        } else {
            context
                .GetCustomerCollectionsIdAsync(&ticket, &user_id)
                .and_then(|op| op.get())?
        };
        Ok(key.to_string())
    }

    /// Developer-defined product id exposed by Microsoft Store as `InAppOfferToken`.
    /// This is the identifier callers see across all platforms — Microsoft-generated
    /// `StoreId` and `SkuStoreId` values stay internal to this module.
    fn app_product_id(store_product: &StoreProduct) -> crate::Result<String> {
        let product_id = store_product.InAppOfferToken()?.to_string();
        if product_id.trim().is_empty() {
            return Err(reject(
                "missingProductId",
                "Windows Store product is missing InAppOfferToken",
            ));
        }
        Ok(product_id)
    }

    /// Extract the product `StoreId` from a SKU `StoreId`.
    ///
    /// Microsoft formats SKU `StoreIds` as `<product StoreId>/<SKU>`, with the
    /// product part being 12 alpha-numeric characters and the SKU 4 — e.g.
    /// `9NBLGGH69M0B/000N`. `ReportConsumableFulfillmentAsync` expects just
    /// the product `StoreId`.
    fn store_id_from_sku_store_id(sku_store_id: &str) -> &str {
        sku_store_id
            .split_once('/')
            .map_or(sku_store_id, |(prefix, _)| prefix)
    }

    /// Query all add-ons associated with this app. We cannot use
    /// `GetStoreProductsAsync` with developer product ids because Microsoft
    /// expects Microsoft-generated `StoreIds` there.
    fn query_associated_products(&self, product_type: &str) -> crate::Result<Vec<StoreProduct>> {
        let context = self.get_store_context()?;

        let product_kinds: Vec<HSTRING> = match product_type {
            "inapp" => vec![
                HSTRING::from("Consumable"),
                HSTRING::from("UnmanagedConsumable"),
            ],
            "subs" => vec![HSTRING::from("Subscription"), HSTRING::from("Durable")],
            _ => vec![
                HSTRING::from("Consumable"),
                HSTRING::from("UnmanagedConsumable"),
                HSTRING::from("Durable"),
                HSTRING::from("Subscription"),
            ],
        };
        let product_kinds: IIterable<HSTRING> = product_kinds.into();

        let query_result = context
            .GetAssociatedStoreProductsAsync(&product_kinds)
            .and_then(|async_op| async_op.get())?;

        let extended_error = query_result.ExtendedError()?;
        if extended_error.is_err() {
            return Err(reject(
                "storeQueryFailed",
                format!(
                    "Store query failed with error: {:?}",
                    extended_error.message()
                ),
            ));
        }

        let products_map = query_result.Products()?;
        let mut products = Vec::new();
        for kv in products_map {
            products.push(kv.Value()?);
        }
        Ok(products)
    }

    #[allow(clippy::unused_async)]
    pub async fn get_products(
        &self,
        product_ids: Vec<String>,
        product_type: String,
    ) -> crate::Result<GetProductsResponse> {
        let store_products = self.query_associated_products(&product_type)?;
        let mut products = Vec::new();

        for requested_id in product_ids {
            let Some(store_product) = store_products
                .iter()
                .find(|sp| Self::app_product_id(sp).is_ok_and(|id| id == requested_id))
            else {
                continue;
            };
            products.push(Self::convert_store_product_to_product(
                store_product,
                &product_type,
            )?);
        }

        Ok(GetProductsResponse { products })
    }

    fn convert_store_product_to_product(
        store_product: &StoreProduct,
        product_type: &str,
    ) -> crate::Result<Product> {
        let product_id = Self::app_product_id(store_product)?;

        let title = store_product.Title()?.to_string();

        let description = store_product.Description()?.to_string();

        let price = store_product.Price()?;

        let formatted_price = price.FormattedPrice()?.to_string();

        let currency_code = price.CurrencyCode()?.to_string();

        // Get the raw price value
        let formatted_base_price = price.FormattedBasePrice()?.to_string();

        // Parse price to get numeric value (remove currency symbols)
        let price_value = formatted_base_price
            .chars()
            .filter(|c| c.is_numeric() || *c == '.')
            .collect::<String>()
            .parse::<f64>()
            .unwrap_or(0.0);

        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let price_amount_micros = (price_value * 1_000_000.0) as i64;

        // Handle subscription offers if this is a subscription product
        let subscription_offer_details = if product_type == "subs" {
            let mut offers = Vec::new();

            // Get SKUs for subscription details
            let skus = store_product.Skus()?;
            let sku_count = skus.Size()?;

            for i in 0..sku_count {
                let sku = skus.GetAt(i)?;

                let sku_id = sku.StoreId()?.to_string();
                let sku_price = sku.Price()?;

                // Check if this SKU has subscription info
                let subscription_info = sku.SubscriptionInfo();

                if let Ok(info) = subscription_info {
                    let billing_period = info.BillingPeriod()?;
                    let billing_period_unit = info.BillingPeriodUnit()?;

                    let billing_period_str = format!(
                        "P{}{}",
                        billing_period,
                        match billing_period_unit.0 {
                            0 => "D", // Day
                            1 => "W", // Week
                            3 => "Y", // Year
                            _ => "M", // Month (default)
                        }
                    );

                    let pricing_phase = PricingPhase {
                        formatted_price: sku_price.FormattedPrice()?.to_string(),
                        price_currency_code: currency_code.clone(),
                        price_amount_micros,
                        billing_period: billing_period_str,
                        billing_cycle_count: 0, // Windows doesn't provide this directly
                        recurrence_mode: 1,     // Infinite recurring
                    };

                    let offer = SubscriptionOffer {
                        offer_token: sku_id.clone(),
                        base_plan_id: sku_id,
                        offer_id: None,
                        pricing_phases: vec![pricing_phase],
                    };

                    offers.push(offer);
                }
            }

            if offers.is_empty() {
                None
            } else {
                Some(offers)
            }
        } else {
            None
        };

        Ok(Product {
            product_id,
            title,
            description,
            product_type: product_type.to_string(),
            formatted_price: Some(formatted_price),
            price_currency_code: Some(currency_code),
            price_amount_micros: Some(price_amount_micros),
            subscription_offer_details,
        })
    }

    #[allow(clippy::unused_async)]
    pub async fn purchase(&self, payload: PurchaseRequest) -> crate::Result<Purchase> {
        let context = self.get_store_context()?;

        // Resolve the developer product id to the matching Windows StoreProduct.
        let store_products = self.query_associated_products(&payload.product_type)?;
        let store_product = store_products
            .into_iter()
            .find(|sp| Self::app_product_id(sp).is_ok_and(|id| id == payload.product_id))
            .ok_or_else(|| {
                reject(
                    "productNotFound",
                    format!("Product not found: {}", payload.product_id),
                )
            })?;
        let product =
            Self::convert_store_product_to_product(&store_product, &payload.product_type)?;
        let store_id = store_product.StoreId()?.to_string();

        // Create purchase properties if we have an offer token (for subscriptions).
        // The offer_token is a SKU StoreId (e.g. `9NXXXX/000N`) which targets a specific SKU.
        // Borrowed (not moved) because the Microsoft b2b credentials
        // on `payload.options` are read again below to mint the Store
        // ID key.
        let offer_token = payload
            .options
            .as_ref()
            .and_then(|opts| opts.offer_token.clone());
        let purchase_result = if let Some(token) = offer_token {
            let properties = StorePurchaseProperties::Create(&HSTRING::from(store_id.as_str()))?;
            properties.SetExtendedJsonData(&HSTRING::from(format!(r#"{{"skuId":"{token}"}}"#)))?;
            context
                .RequestPurchaseWithPurchasePropertiesAsync(
                    &HSTRING::from(store_id.as_str()),
                    &properties,
                )
                .and_then(|async_op| async_op.get())?
        } else {
            context
                .RequestPurchaseAsync(&HSTRING::from(store_id.as_str()))
                .and_then(|async_op| async_op.get())?
        };

        let status = purchase_result.Status()?;
        let purchase_state = match status {
            StorePurchaseStatus::Succeeded | StorePurchaseStatus::AlreadyPurchased => {
                PurchaseStateValue::Purchased
            }
            StorePurchaseStatus::NotPurchased => {
                return Err(reject("purchaseNotCompleted", "Purchase was not completed"));
            }
            StorePurchaseStatus::NetworkError => {
                return Err(reject("networkError", "Network error during purchase"));
            }
            StorePurchaseStatus::ServerError => {
                return Err(reject("serverError", "Server error during purchase"));
            }
            _ => {
                return Err(reject("purchaseFailed", "Purchase failed"));
            }
        };

        // Get extended error info if available
        let error_message = purchase_result
            .ExtendedError()
            .ok()
            .map_or_else(String::new, windows::core::HRESULT::message);

        let purchase_time = FileTime::now().to_unix_time_millis();
        let purchase_token = WindowsPurchaseTokenV1::new(store_id, purchase_time)?.encode()?;

        // Mint a Store ID key when the caller supplied Microsoft b2b
        // credentials. Caller is presumed to have already fetched the
        // service ticket from its backend (the publisher's Entra
        // app) — this plugin only relays it to the WinRT API. Either
        // field absent → field stays `None` (existing behaviour).
        let jws_representation = if let (Some(ticket), Some(user_id)) = (
            payload
                .options
                .as_ref()
                .and_then(|o| o.service_ticket.as_deref()),
            payload
                .options
                .as_ref()
                .and_then(|o| o.publisher_user_id.as_deref()),
        ) {
            Some(self.mint_store_id_key(&payload.product_type, ticket, user_id)?)
        } else {
            None
        };

        let purchase = Purchase {
            order_id: Some(purchase_token.clone()),
            package_name: product.title.clone(),
            product_id: product.product_id.clone(),
            purchase_time,
            purchase_token,
            purchase_state,
            is_auto_renewing: product.product_type == "subs",
            is_acknowledged: true, // Windows Store handles acknowledgment
            original_json: format!(
                r#"{{"status":{},"message":"{}","productId":"{}"}}"#,
                status.0, error_message, product.product_id
            ),
            signature: String::new(), // Windows doesn't provide signatures like Android
            original_id: None, // Windows doesn't have original transaction IDs like iOS/macOS
            jws_representation,
        };

        self.trigger("purchaseUpdated", purchase.clone());
        Ok(purchase)
    }

    #[allow(clippy::unused_async)]
    pub async fn restore_purchases(
        &self,
        request: RestorePurchasesRequest,
    ) -> crate::Result<RestorePurchasesResponse> {
        let context = self.get_store_context()?;

        // Get app license info
        let app_license = context
            .GetAppLicenseAsync()
            .and_then(|async_op| async_op.get())?;

        // Microsoft issues one Store ID key per user that covers every
        // subscription / IAP, so mint it once and stamp it onto every
        // returned purchase. Minting per-row would burn calls for no
        // benefit — the backend's recurrence/collections query would
        // resolve the same set either way.
        let jws_representation = if let (Some(ticket), Some(user_id)) = (
            request.service_ticket.as_deref(),
            request.publisher_user_id.as_deref(),
        ) {
            Some(self.mint_store_id_key(&request.product_type, ticket, user_id)?)
        } else {
            None
        };

        let mut purchases = Vec::new();

        // Get add-on licenses (in-app purchases)
        let addon_licenses = app_license.AddOnLicenses()?;

        for kv in addon_licenses {
            let license = kv.Value()?;
            let mut purchase = self.convert_license_to_purchase(&license, &request.product_type)?;
            purchase.jws_representation.clone_from(&jws_representation);

            if purchase.purchase_state == PurchaseStateValue::Purchased {
                purchases.push(purchase);
            }
        }

        Ok(RestorePurchasesResponse { purchases })
    }

    fn convert_license_to_purchase(
        &self,
        license: &StoreLicense,
        product_type: &str,
    ) -> crate::Result<Purchase> {
        let product_id = license.InAppOfferToken()?.to_string();
        let sku_store_id = license.SkuStoreId()?.to_string();
        // ReportConsumableFulfillmentAsync needs the product StoreId, which is
        // the prefix of the SKU StoreId returned by the license.
        let store_id = Self::store_id_from_sku_store_id(&sku_store_id).to_string();
        let is_active = license.IsActive()?;
        let expiration_millis = Self::datetime_to_unix_millis(license.ExpirationDate()?);

        // Estimate purchase time (30 days before expiration for monthly subs)
        let purchase_time = if product_type == "subs" && expiration_millis > 0 {
            expiration_millis - (30 * 24 * 60 * 60 * 1000)
        } else {
            FileTime::now().to_unix_time_millis()
        };

        let purchase_token = WindowsPurchaseTokenV1::new(store_id, purchase_time)?.encode()?;

        let purchase_state = if is_active {
            PurchaseStateValue::Purchased
        } else {
            PurchaseStateValue::Canceled
        };

        Ok(Purchase {
            order_id: Some(purchase_token.clone()),
            package_name: self.app_handle.package_info().name.clone(),
            product_id,
            purchase_time,
            purchase_token,
            purchase_state,
            is_auto_renewing: product_type == "subs" && is_active,
            is_acknowledged: true,
            original_json: format!(
                r#"{{"isActive":{is_active},"expirationDate":{expiration_millis}}}"#
            ),
            signature: String::new(),
            original_id: None,
            jws_representation: None, // Windows doesn't have JWS like iOS/macOS
        })
    }

    /// No-op: Microsoft Store auto-acknowledges purchases. Method exists for API parity.
    #[allow(clippy::unused_async, clippy::unused_self)]
    pub async fn acknowledge_purchase(&self, _purchase_token: String) -> crate::Result<()> {
        Ok(())
    }

    #[allow(clippy::unused_async)]
    pub async fn consume_purchase(&self, purchase_token: String) -> crate::Result<()> {
        let envelope = WindowsPurchaseTokenV1::decode(&purchase_token)?;
        let context = self.get_store_context()?;
        let store_id = HSTRING::from(&envelope.store_id);
        let tracking_id = windows::core::GUID::new()?;

        let result = context
            .ReportConsumableFulfillmentAsync(&store_id, 1u32, tracking_id)
            .and_then(|async_op| async_op.get())?;

        match result.Status()? {
            StoreConsumableStatus::Succeeded => Ok(()),
            StoreConsumableStatus::InsufficentQuantity => Err(reject(
                "insufficientQuantity",
                "Not enough balance remaining to consume",
            )),
            StoreConsumableStatus::NetworkError => {
                Err(reject("networkError", "Network error during consume"))
            }
            StoreConsumableStatus::ServerError => {
                Err(reject("serverError", "Server error during consume"))
            }
            _ => Err(reject("consumeFailed", "Failed to consume purchase")),
        }
    }

    #[allow(clippy::unused_async)]
    pub async fn get_product_status(
        &self,
        product_id: String,
        product_type: String,
    ) -> crate::Result<ProductStatus> {
        let context = self.get_store_context()?;

        // Get app license to check ownership
        let app_license = context
            .GetAppLicenseAsync()
            .and_then(|async_op| async_op.get())?;

        let addon_licenses = app_license.AddOnLicenses()?;

        // AddOnLicenses is keyed by SKU StoreId, not by developer product id,
        // so we cannot use HasKey/Lookup with the requested product_id.
        // Iterate instead and match on InAppOfferToken.
        for kv in addon_licenses {
            let license = kv.Value()?;
            if license.InAppOfferToken()? != product_id {
                continue;
            }

            let is_active = license.IsActive()?;
            let expiration_time = Self::datetime_to_unix_millis(license.ExpirationDate()?);
            let sku_store_id = license.SkuStoreId()?.to_string();
            let store_id = Self::store_id_from_sku_store_id(&sku_store_id).to_string();

            let purchase_time = if product_type == "subs" && expiration_time > 0 {
                expiration_time - (30 * 24 * 60 * 60 * 1000)
            } else {
                expiration_time
            };

            let purchase_token = WindowsPurchaseTokenV1::new(store_id, purchase_time)?.encode()?;

            let purchase_state = if is_active {
                Some(PurchaseStateValue::Purchased)
            } else {
                Some(PurchaseStateValue::Canceled)
            };

            return Ok(ProductStatus {
                product_id,
                is_owned: is_active,
                purchase_state,
                purchase_time: Some(purchase_time),
                expiration_time: if expiration_time > 0 {
                    Some(expiration_time)
                } else {
                    None
                },
                is_auto_renewing: Some(product_type == "subs" && is_active),
                is_acknowledged: Some(true),
                purchase_token: Some(purchase_token),
            });
        }

        Ok(ProductStatus {
            product_id,
            is_owned: false,
            purchase_state: None,
            purchase_time: None,
            expiration_time: None,
            is_auto_renewing: None,
            is_acknowledged: None,
            purchase_token: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datetime_to_unix_millis_epoch() {
        // Unix epoch: January 1, 1970 00:00:00 UTC
        // In Windows ticks: 116444736000000000 (100-nanosecond intervals since Jan 1, 1601)
        let datetime = DateTime {
            UniversalTime: 116_444_736_000_000_000,
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(datetime);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_datetime_to_unix_millis_known_date() {
        // November 14, 2023 00:00:00 UTC
        // Unix timestamp: 1699920000000 ms
        // Windows ticks: 133445856000000000
        let datetime = DateTime {
            UniversalTime: 133_445_856_000_000_000,
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(datetime);
        assert_eq!(result, 1_699_920_000_000);
    }

    #[test]
    fn test_datetime_to_unix_millis_before_epoch() {
        // Date before Unix epoch should give negative result
        // January 1, 1969 00:00:00 UTC
        // Windows ticks: 116413200000000000
        let datetime = DateTime {
            UniversalTime: 116_413_200_000_000_000,
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(datetime);
        assert!(result < 0);
    }

    #[test]
    fn test_datetime_to_unix_millis_year_2000() {
        // January 1, 2000 00:00:00 UTC
        // Unix timestamp: 946684800000 ms
        // Windows ticks: 125911584000000000
        let datetime = DateTime {
            UniversalTime: 125_911_584_000_000_000,
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(datetime);
        assert_eq!(result, 946_684_800_000);
    }

    #[test]
    fn test_datetime_to_unix_millis_precision() {
        // Sub-second precision is preserved down to milliseconds.
        let datetime = DateTime {
            UniversalTime: 116_444_736_000_000_000 + 5_000_000, // epoch + 500ms in 100-ns ticks
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(datetime);
        assert_eq!(result, 500);
    }

    #[test]
    fn test_datetime_to_unix_millis_one_second_after_epoch() {
        // 1 second after Unix epoch
        let datetime = DateTime {
            UniversalTime: 116_444_736_000_000_000 + 10_000_000, // epoch + 1 second in 100-ns ticks
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(datetime);
        assert_eq!(result, 1000);
    }

    #[test]
    fn test_datetime_to_unix_millis_far_future() {
        // January 1, 2100 00:00:00 UTC
        // Windows ticks: 157766880000000000
        let datetime = DateTime {
            UniversalTime: 157_766_880_000_000_000,
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(datetime);
        // Should be approximately 4102444800000 ms
        assert!(result > 4_000_000_000_000);
    }

    const SAMPLE_TRACKING_ID: &str = "00000000-0000-0000-0000-00000000002a";

    fn sample_envelope(store_id: &str) -> WindowsPurchaseTokenV1 {
        WindowsPurchaseTokenV1::new(store_id.to_string(), 1_714_387_200_000)
            .expect("GUID::new must succeed in tests")
    }

    fn assert_envelope_eq(a: &WindowsPurchaseTokenV1, b: &WindowsPurchaseTokenV1) {
        assert_eq!(a.v, b.v);
        assert_eq!(a.store_id, b.store_id);
        assert_eq!(a.purchase_time, b.purchase_time);
        assert_eq!(a.tracking_id, b.tracking_id);
    }

    #[test]
    fn test_envelope_round_trip_typical() {
        let original = sample_envelope("9MSPC6MP8FM4");
        let encoded = original.encode().expect("encode must succeed");
        let decoded =
            WindowsPurchaseTokenV1::decode(&encoded).expect("just-encoded token must decode");
        assert_envelope_eq(&original, &decoded);
    }

    #[test]
    fn test_envelope_decode_rejects_empty_store_id() {
        let encoded = sample_envelope("").encode().expect("encode must succeed");
        assert!(WindowsPurchaseTokenV1::decode(&encoded).is_err());
    }

    #[test]
    fn test_envelope_round_trip_long_store_id() {
        let original = sample_envelope(&"x".repeat(512));
        let encoded = original.encode().expect("encode must succeed");
        let decoded =
            WindowsPurchaseTokenV1::decode(&encoded).expect("just-encoded token must decode");
        assert_envelope_eq(&original, &decoded);
    }

    #[test]
    fn test_envelope_encoded_uses_url_safe_alphabet() {
        let encoded = sample_envelope("9MSPC6MP8FM4")
            .encode()
            .expect("encode must succeed");
        for ch in encoded.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "encoded token contains non-URL-safe char: {ch:?}"
            );
        }
    }

    #[test]
    fn test_envelope_encoding_is_stable() {
        let env = sample_envelope("9MSPC6MP8FM4");
        let a = env.encode().expect("encode must succeed");
        let b = env.encode().expect("encode must succeed");
        assert_eq!(a, b);
    }

    #[test]
    fn test_envelope_decode_rejects_malformed_base64() {
        assert!(WindowsPurchaseTokenV1::decode("!!!not-base64!!!").is_err());
    }

    #[test]
    fn test_envelope_decode_rejects_non_json_payload() {
        // Valid base64 of "hello world" (not JSON).
        let bogus = URL_SAFE_NO_PAD.encode(b"hello world");
        assert!(WindowsPurchaseTokenV1::decode(&bogus).is_err());
    }

    #[test]
    fn test_envelope_decode_rejects_unknown_version() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "v": 99,
            "store_id": "9MSPC6MP8FM4",
            "purchase_time": 1_714_387_200_000_i64,
            "tracking_id": SAMPLE_TRACKING_ID,
        }))
        .expect("static JSON must serialize");
        let encoded = URL_SAFE_NO_PAD.encode(&bytes);
        assert!(WindowsPurchaseTokenV1::decode(&encoded).is_err());
    }

    #[test]
    fn test_envelope_decode_rejects_missing_store_id() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "v": 1,
            "purchase_time": 1_714_387_200_000_i64,
            "tracking_id": SAMPLE_TRACKING_ID,
        }))
        .expect("static JSON must serialize");
        let encoded = URL_SAFE_NO_PAD.encode(&bytes);
        assert!(WindowsPurchaseTokenV1::decode(&encoded).is_err());
    }

    #[test]
    fn test_envelope_decode_rejects_missing_tracking_id() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "v": 1,
            "store_id": "9MSPC6MP8FM4",
            "purchase_time": 1_714_387_200_000_i64,
        }))
        .expect("static JSON must serialize");
        let encoded = URL_SAFE_NO_PAD.encode(&bytes);
        assert!(WindowsPurchaseTokenV1::decode(&encoded).is_err());
    }

    #[test]
    fn test_envelope_decode_rejects_empty_tracking_id() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "v": 1,
            "store_id": "9MSPC6MP8FM4",
            "purchase_time": 1_714_387_200_000_i64,
            "tracking_id": "",
        }))
        .expect("static JSON must serialize");
        let encoded = URL_SAFE_NO_PAD.encode(&bytes);
        assert!(WindowsPurchaseTokenV1::decode(&encoded).is_err());
    }

    #[test]
    fn test_store_id_from_sku_store_id_strips_sku() {
        assert_eq!(
            Iap::<tauri::Wry>::store_id_from_sku_store_id("9MSPC6MP8FM4/000N"),
            "9MSPC6MP8FM4"
        );
    }

    #[test]
    fn test_store_id_from_sku_store_id_passthrough_when_no_slash() {
        assert_eq!(
            Iap::<tauri::Wry>::store_id_from_sku_store_id("9MSPC6MP8FM4"),
            "9MSPC6MP8FM4"
        );
    }

    #[test]
    fn test_store_id_from_sku_store_id_leading_slash_returns_empty() {
        // Malformed input with no product prefix returns the empty prefix
        // rather than silently masking the bad data.
        assert_eq!(Iap::<tauri::Wry>::store_id_from_sku_store_id("/000N"), "");
    }
}
