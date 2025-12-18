use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Iap<R>> {
    Ok(Iap(app.clone()))
}

/// Access to the iap APIs.
pub struct Iap<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Iap<R> {
    pub fn initialize(&self) -> crate::Result<InitializeResponse> {
        Err(crate::Error::from(std::io::Error::other(
            "IAP is not supported on this platform",
        )))
    }

    pub async fn get_products(
        &self,
        _product_ids: Vec<String>,
        _product_type: String,
    ) -> crate::Result<GetProductsResponse> {
        Err(crate::Error::from(std::io::Error::other(
            "IAP is not supported on this platform",
        )))
    }

    pub async fn purchase(&self, _payload: PurchaseRequest) -> crate::Result<Purchase> {
        Err(crate::Error::from(std::io::Error::other(
            "IAP is not supported on this platform",
        )))
    }

    pub async fn restore_purchases(
        &self,
        _product_type: String,
    ) -> crate::Result<RestorePurchasesResponse> {
        Err(crate::Error::from(std::io::Error::other(
            "IAP is not supported on this platform",
        )))
    }

    pub fn get_purchase_history(&self) -> crate::Result<GetPurchaseHistoryResponse> {
        Err(crate::Error::from(std::io::Error::other(
            "IAP is not supported on this platform",
        )))
    }

    pub async fn acknowledge_purchase(
        &self,
        _purchase_token: String,
    ) -> crate::Result<AcknowledgePurchaseResponse> {
        Err(crate::Error::from(std::io::Error::other(
            "IAP is not supported on this platform",
        )))
    }

    pub async fn get_product_status(
        &self,
        _product_id: String,
        _product_type: String,
    ) -> crate::Result<ProductStatus> {
        Err(crate::Error::from(std::io::Error::other(
            "IAP is not supported on this platform",
        )))
    }
}
