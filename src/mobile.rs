use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::*;

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "app.tauri.iap";

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_iap);

// initializes the Kotlin or Swift plugin classes
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<Iap<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "IapPlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_iap)?;

    Ok(Iap(handle))
}

/// Access to the iap APIs.
pub struct Iap<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Iap<R> {
    pub fn initialize(&self) -> crate::Result<InitializeResponse> {
        self.0
            .run_mobile_plugin("initialize", InitializeRequest {})
            .map_err(Into::into)
    }

    pub async fn get_products(
        &self,
        product_ids: Vec<String>,
        product_type: String,
    ) -> crate::Result<GetProductsResponse> {
        self.0
            .run_mobile_plugin_async(
                "getProducts",
                GetProductsRequest {
                    product_ids,
                    product_type,
                },
            )
            .await
            .map_err(Into::into)
    }

    pub async fn purchase(&self, payload: PurchaseRequest) -> crate::Result<Purchase> {
        self.0
            .run_mobile_plugin_async("purchase", payload)
            .await
            .map_err(Into::into)
    }

    pub async fn restore_purchases(
        &self,
        product_type: String,
    ) -> crate::Result<RestorePurchasesResponse> {
        self.0
            .run_mobile_plugin_async("restorePurchases", RestorePurchasesRequest { product_type })
            .await
            .map_err(Into::into)
    }

    pub fn get_purchase_history(&self) -> crate::Result<GetPurchaseHistoryResponse> {
        self.0
            .run_mobile_plugin("getPurchaseHistory", ())
            .map_err(Into::into)
    }

    pub async fn acknowledge_purchase(
        &self,
        purchase_token: String,
    ) -> crate::Result<AcknowledgePurchaseResponse> {
        self.0
            .run_mobile_plugin_async(
                "acknowledgePurchase",
                AcknowledgePurchaseRequest { purchase_token },
            )
            .await
            .map_err(Into::into)
    }

    pub async fn get_product_status(
        &self,
        product_id: String,
        product_type: String,
    ) -> crate::Result<ProductStatus> {
        self.0
            .run_mobile_plugin_async(
                "getProductStatus",
                GetProductStatusRequest {
                    product_id,
                    product_type,
                },
            )
            .await
            .map_err(Into::into)
    }
}
