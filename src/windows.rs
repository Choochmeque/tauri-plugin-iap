use serde::de::DeserializeOwned;
use tauri::Emitter;
use tauri::Manager;
use tauri::{plugin::PluginApi, AppHandle, Runtime};
use windows::core::{Interface, HSTRING};
use windows::{
    Foundation::DateTime,
    Services::Store::{
        StoreContext, StoreLicense, StoreProduct, StorePurchaseProperties, StorePurchaseStatus,
    },
    Win32::UI::Shell::IInitializeWithWindow,
};
use windows_collections::IIterable;

use crate::error::{ErrorResponse, PluginInvokeError};
use crate::models::*;
use std::sync::{Arc, RwLock};

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
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
    /// Get or create the StoreContext instance
    fn get_store_context(&self) -> crate::Result<StoreContext> {
        let mut context_guard = self.store_context.write().map_err(|e| {
            crate::Error::PluginInvoke(PluginInvokeError::InvokeRejected(ErrorResponse {
                code: Some("internalError".to_string()),
                message: Some(format!("Failed to acquire write lock: {:?}", e)),
                data: (),
            }))
        })?;

        if context_guard.is_none() {
            // Get the default store context for the current user
            let context = StoreContext::GetDefault()?;

            let window = self.app_handle.get_webview_window("main").ok_or_else(|| {
                crate::Error::PluginInvoke(PluginInvokeError::InvokeRejected(ErrorResponse {
                    code: Some("windowError".to_string()),
                    message: Some("Failed to get main window".to_string()),
                    data: (),
                }))
            })?;
            let hwnd = window.hwnd().map_err(|e| {
                crate::Error::PluginInvoke(PluginInvokeError::InvokeRejected(ErrorResponse {
                    code: Some("windowError".to_string()),
                    message: Some(format!("Failed to get window handle: {:?}", e)),
                    data: (),
                }))
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
            .ok_or_else(|| {
                crate::Error::PluginInvoke(PluginInvokeError::InvokeRejected(ErrorResponse {
                    code: Some("storeNotInitialized".to_string()),
                    message: Some("Store context not initialized".to_string()),
                    data: (),
                }))
            })?
            .clone())
    }

    /// Convert Windows DateTime to Unix timestamp in milliseconds
    fn datetime_to_unix_millis(datetime: &DateTime) -> i64 {
        // Windows DateTime is in 100-nanosecond intervals since January 1, 1601
        // Convert to Unix timestamp (milliseconds since January 1, 1970)
        const WINDOWS_TICK: i64 = 10000000;
        const SEC_TO_UNIX_EPOCH: i64 = 11644473600;

        let windows_ticks = datetime.UniversalTime;
        let seconds_since_1601 = windows_ticks / WINDOWS_TICK;
        let unix_seconds = seconds_since_1601 - SEC_TO_UNIX_EPOCH;
        unix_seconds * 1000 // Convert to milliseconds
    }

    /// Emit an event to the frontend (equivalent to iOS/Android `trigger` method).
    fn trigger<S: serde::Serialize + Clone>(&self, event: &str, payload: S) {
        let _ = self.app_handle.emit(event, payload);
    }

    pub fn initialize(&self) -> crate::Result<InitializeResponse> {
        let _ = self.get_store_context()?;
        Ok(InitializeResponse { success: true })
    }

    pub async fn get_products(
        &self,
        product_ids: Vec<String>,
        product_type: String,
    ) -> crate::Result<GetProductsResponse> {
        let context = self.get_store_context()?;

        // Convert product IDs to HSTRING
        let store_ids: Vec<HSTRING> = product_ids
            .iter()
            .map(|id| HSTRING::from(id.as_str()))
            .collect();

        // Determine product kinds based on type
        let product_kinds: Vec<HSTRING> = match product_type.as_str() {
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

        let store_ids: IIterable<HSTRING> = store_ids.into();
        let product_kinds: IIterable<HSTRING> = product_kinds.into();

        // Query products from the store
        let query_result = context
            .GetStoreProductsAsync(&product_kinds, &store_ids)
            .and_then(|async_op| async_op.get())?;

        // Check for any errors
        let extended_error = query_result.ExtendedError()?;
        if extended_error.is_err() {
            return Err(crate::Error::PluginInvoke(
                PluginInvokeError::InvokeRejected(ErrorResponse {
                    code: Some("storeQueryFailed".to_string()),
                    message: Some(format!(
                        "Store query failed with error: {:?}",
                        extended_error.message()
                    )),
                    data: (),
                }),
            ));
        }

        let products_map = query_result.Products()?;
        let mut products = Vec::new();

        // Iterate through the products
        let iterator = products_map.First()?;
        while iterator.HasCurrent()? {
            let item = iterator.Current()?;
            let store_product = item.Value()?;

            let product = self.convert_store_product_to_product(&store_product, &product_type)?;
            products.push(product);

            iterator.MoveNext()?;
        }

        Ok(GetProductsResponse { products })
    }

    fn convert_store_product_to_product(
        &self,
        store_product: &StoreProduct,
        product_type: &str,
    ) -> crate::Result<Product> {
        let product_id = store_product.StoreId()?.to_string();

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
                sku.StoreId()?.to_string();

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
                            2 => "M", // Month
                            3 => "Y", // Year
                            _ => "M",
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

            if !offers.is_empty() {
                Some(offers)
            } else {
                None
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

    pub async fn purchase(&self, payload: PurchaseRequest) -> crate::Result<Purchase> {
        let context = self.get_store_context()?;

        // Get the product first to ensure it exists
        let products_response = self
            .get_products(
                vec![payload.product_id.clone()],
                payload.product_type.clone(),
            )
            .await?;

        if products_response.products.is_empty() {
            return Err(crate::Error::PluginInvoke(
                PluginInvokeError::InvokeRejected(ErrorResponse {
                    code: Some("productNotFound".to_string()),
                    message: Some("Product not found".to_string()),
                    data: (),
                }),
            ));
        }

        let product = &products_response.products[0];
        let product_title = product.title.clone();

        let store_id = HSTRING::from(&payload.product_id);

        // Create purchase properties if we have an offer token (for subscriptions)
        let offer_token = payload.options.and_then(|opts| opts.offer_token);
        let purchase_result = if let Some(token) = offer_token {
            let properties = StorePurchaseProperties::Create(&HSTRING::from(&payload.product_id))?;

            // Set the SKU ID for subscription offers
            properties
                .SetExtendedJsonData(&HSTRING::from(format!(r#"{{"skuId":"{}"}}"#, token)))?;

            context
                .RequestPurchaseWithPurchasePropertiesAsync(&store_id, &properties)
                .and_then(|async_op| async_op.get())?
        } else {
            // Simple purchase without properties
            context
                .RequestPurchaseAsync(&store_id)
                .and_then(|async_op| async_op.get())?
        };

        // Check purchase status
        let status = purchase_result.Status()?;

        let purchase_state = match status {
            StorePurchaseStatus::Succeeded => PurchaseStateValue::Purchased,
            StorePurchaseStatus::AlreadyPurchased => PurchaseStateValue::Purchased,
            StorePurchaseStatus::NotPurchased => {
                return Err(crate::Error::PluginInvoke(
                    PluginInvokeError::InvokeRejected(ErrorResponse {
                        code: Some("purchaseNotCompleted".to_string()),
                        message: Some("Purchase was not completed".to_string()),
                        data: (),
                    }),
                ));
            }
            StorePurchaseStatus::NetworkError => {
                return Err(crate::Error::PluginInvoke(
                    PluginInvokeError::InvokeRejected(ErrorResponse {
                        code: Some("networkError".to_string()),
                        message: Some("Network error during purchase".to_string()),
                        data: (),
                    }),
                ));
            }
            StorePurchaseStatus::ServerError => {
                return Err(crate::Error::PluginInvoke(
                    PluginInvokeError::InvokeRejected(ErrorResponse {
                        code: Some("serverError".to_string()),
                        message: Some("Server error during purchase".to_string()),
                        data: (),
                    }),
                ));
            }
            _ => {
                return Err(crate::Error::PluginInvoke(
                    PluginInvokeError::InvokeRejected(ErrorResponse {
                        code: Some("purchaseFailed".to_string()),
                        message: Some("Purchase failed".to_string()),
                        data: (),
                    }),
                ));
            }
        };

        // Get extended error info if available
        let extended_error = purchase_result.ExtendedError().ok();
        let error_message = if let Some(error) = extended_error {
            error.message()
        } else {
            String::new()
        };

        // Generate purchase details
        let purchase_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| {
                crate::Error::PluginInvoke(PluginInvokeError::InvokeRejected(ErrorResponse {
                    code: Some("systemTimeError".to_string()),
                    message: Some(format!("Failed to get system time: {:?}", e)),
                    data: (),
                }))
            })?
            .as_millis() as i64;

        let purchase_token = format!("win_{}_{}", product.product_id, purchase_time);

        let purchase = Purchase {
            order_id: Some(purchase_token.clone()),
            package_name: product_title,
            product_id: product.product_id.clone(),
            purchase_time,
            purchase_token: purchase_token.clone(),
            purchase_state,
            is_auto_renewing: product.product_type == "subs",
            is_acknowledged: true, // Windows Store handles acknowledgment
            original_json: format!(
                r#"{{"status":{},"message":"{}","productId":"{}"}}"#,
                status.0, error_message, product.product_id
            ),
            signature: String::new(), // Windows doesn't provide signatures like Android
            original_id: None, // Windows doesn't have original transaction IDs like iOS/macOS
        };

        // Emit event for purchase state change
        self.trigger("purchaseUpdated", purchase.clone());

        Ok(purchase)
    }

    pub async fn restore_purchases(
        &self,
        product_type: String,
    ) -> crate::Result<RestorePurchasesResponse> {
        let context = self.get_store_context()?;

        // Get app license info
        let app_license = context
            .GetAppLicenseAsync()
            .and_then(|async_op| async_op.get())?;

        let mut purchases = Vec::new();

        // Get add-on licenses (in-app purchases)
        let addon_licenses = app_license.AddOnLicenses()?;

        let iterator = addon_licenses.First()?;
        while iterator.HasCurrent()? {
            let item = iterator.Current()?;
            let license = item.Value()?;

            let purchase = self.convert_license_to_purchase(&license, &product_type)?;

            if purchase.purchase_state == PurchaseStateValue::Purchased {
                purchases.push(purchase);
            }

            iterator.MoveNext()?;
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

        let is_active = license.IsActive()?;

        let expiration_date = license.ExpirationDate()?;
        let expiration_millis = Self::datetime_to_unix_millis(&expiration_date);

        // Estimate purchase time (30 days before expiration for monthly subs)
        let purchase_time = if product_type == "subs" && expiration_millis > 0 {
            expiration_millis - (30 * 24 * 60 * 60 * 1000)
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| {
                    crate::Error::PluginInvoke(PluginInvokeError::InvokeRejected(ErrorResponse {
                        code: Some("systemTimeError".to_string()),
                        message: Some(format!("Failed to get system time: {:?}", e)),
                        data: (),
                    }))
                })?
                .as_millis() as i64
        };

        let purchase_state = if is_active {
            PurchaseStateValue::Purchased
        } else {
            PurchaseStateValue::Canceled
        };

        Ok(Purchase {
            order_id: Some(sku_store_id.clone()),
            package_name: self.app_handle.package_info().name.clone(),
            product_id,
            purchase_time,
            purchase_token: sku_store_id,
            purchase_state,
            is_auto_renewing: product_type == "subs" && is_active,
            is_acknowledged: true,
            original_json: format!(
                r#"{{"isActive":{},"expirationDate":{}}}"#,
                is_active, expiration_millis
            ),
            signature: String::new(),
            original_id: None,
        })
    }

    pub async fn acknowledge_purchase(
        &self,
        _purchase_token: String,
    ) -> crate::Result<AcknowledgePurchaseResponse> {
        // Windows Store handles acknowledgment automatically
        // This method exists for API compatibility
        Ok(AcknowledgePurchaseResponse { success: true })
    }

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

        // Look for the specific product license
        let product_key = HSTRING::from(&product_id);
        let has_license = addon_licenses.HasKey(&product_key)?;

        if has_license {
            let license = addon_licenses.Lookup(&product_key)?;

            let is_active = license.IsActive()?;
            let expiration_date = license.ExpirationDate()?;
            let expiration_time = Self::datetime_to_unix_millis(&expiration_date);

            let purchase_time = if product_type == "subs" && expiration_time > 0 {
                expiration_time - (30 * 24 * 60 * 60 * 1000)
            } else {
                expiration_time
            };

            let purchase_state = if is_active {
                Some(PurchaseStateValue::Purchased)
            } else {
                Some(PurchaseStateValue::Canceled)
            };

            let sku_store_id = license.SkuStoreId()?.to_string();

            Ok(ProductStatus {
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
                purchase_token: Some(sku_store_id),
            })
        } else {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datetime_to_unix_millis_epoch() {
        // Unix epoch: January 1, 1970 00:00:00 UTC
        // In Windows ticks: 116444736000000000 (100-nanosecond intervals since Jan 1, 1601)
        let datetime = DateTime {
            UniversalTime: 116444736000000000,
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(&datetime);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_datetime_to_unix_millis_known_date() {
        // November 14, 2023 00:00:00 UTC
        // Unix timestamp: 1699920000000 ms
        // Windows ticks: 133445856000000000
        let datetime = DateTime {
            UniversalTime: 133445856000000000,
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(&datetime);
        assert_eq!(result, 1699920000000);
    }

    #[test]
    fn test_datetime_to_unix_millis_before_epoch() {
        // Date before Unix epoch should give negative result
        // January 1, 1969 00:00:00 UTC
        // Windows ticks: 116413200000000000
        let datetime = DateTime {
            UniversalTime: 116413200000000000,
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(&datetime);
        assert!(result < 0);
    }

    #[test]
    fn test_datetime_to_unix_millis_year_2000() {
        // January 1, 2000 00:00:00 UTC
        // Unix timestamp: 946684800000 ms
        // Windows ticks: 125911584000000000
        let datetime = DateTime {
            UniversalTime: 125911584000000000,
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(&datetime);
        assert_eq!(result, 946684800000);
    }

    #[test]
    fn test_datetime_to_unix_millis_precision() {
        // Test that sub-second precision is handled correctly (truncated to seconds then converted to ms)
        // The function converts to seconds first, losing sub-second precision
        let datetime = DateTime {
            UniversalTime: 116444736000000000 + 5000000, // epoch + 500ms in 100-ns ticks
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(&datetime);
        // Since we divide by WINDOWS_TICK (10_000_000), we truncate sub-second values
        assert_eq!(result, 0);
    }

    #[test]
    fn test_datetime_to_unix_millis_one_second_after_epoch() {
        // 1 second after Unix epoch
        let datetime = DateTime {
            UniversalTime: 116444736000000000 + 10000000, // epoch + 1 second in 100-ns ticks
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(&datetime);
        assert_eq!(result, 1000);
    }

    #[test]
    fn test_datetime_to_unix_millis_far_future() {
        // January 1, 2100 00:00:00 UTC
        // Windows ticks: 157766880000000000
        let datetime = DateTime {
            UniversalTime: 157766880000000000,
        };
        let result = Iap::<tauri::Wry>::datetime_to_unix_millis(&datetime);
        // Should be approximately 4102444800000 ms
        assert!(result > 4000000000000);
    }
}
