use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

/// Validation checks for macOS IAP functionality.
///
/// StoreKit requires the app to run from a signed .app bundle to communicate
/// with the App Store. During development with `tauri dev`, the binary runs
/// directly without a bundle, causing StoreKit calls to fail silently or crash.
mod validation {
    /// Ensures the app is running from a .app bundle.
    pub fn require_bundle() -> crate::Result<()> {
        std::env::current_exe()
            .ok()
            .and_then(|exe| {
                let macos = exe.parent()?;
                let contents = macos.parent()?;
                let bundle = contents.parent()?;
                (macos.ends_with("MacOS")
                    && contents.ends_with("Contents")
                    && bundle.to_string_lossy().ends_with(".app"))
                .then_some(())
            })
            .ok_or_else(|| {
                crate::error::PluginInvokeError::InvokeRejected(crate::error::ErrorResponse {
                    code: None,
                    message: Some("IAP requires the app to run from a .app bundle.".to_string()),
                    data: (),
                })
                .into()
            })
    }
}

#[swift_bridge::bridge]
mod ffi {
    pub enum FFIResult {
        Err(String), // error message from Swift
    }

    extern "Rust" {
        fn trigger(event: String, payload: String) -> Result<(), FFIResult>;
    }

    extern "Swift" {
        #[swift_bridge(Sendable)]
        type IapPlugin;
        #[swift_bridge(init, swift_name = "initPlugin")]
        fn init_plugin() -> IapPlugin;

        async fn getProducts(
            &self,
            productIds: Vec<String>,
            productType: String,
        ) -> Result<String, FFIResult>;
        async fn purchase(
            &self,
            productId: String,
            productType: String,
            offerToken: Option<String>,
        ) -> Result<String, FFIResult>;
        async fn restorePurchases(&self, productType: String) -> Result<String, FFIResult>;
        async fn acknowledgePurchase(&self, purchaseToken: String) -> Result<String, FFIResult>;
        async fn getProductStatus(
            &self,
            productId: String,
            productType: String,
        ) -> Result<String, FFIResult>;
    }
}

/// Extension trait for parsing FFI responses from Swift into typed Rust results.
trait ParseFfiResponse {
    /// Deserializes a JSON response into the target type, converting FFI errors
    /// into plugin errors.
    fn parse<T: DeserializeOwned>(self) -> crate::Result<T>;
}

impl ParseFfiResponse for Result<String, ffi::FFIResult> {
    fn parse<T: DeserializeOwned>(self) -> crate::Result<T> {
        match self {
            Ok(json) => serde_json::from_str(&json)
                .map_err(|e| crate::error::PluginInvokeError::CannotDeserializeResponse(e).into()),
            Err(ffi::FFIResult::Err(msg)) => Err(crate::error::PluginInvokeError::InvokeRejected(
                crate::error::ErrorResponse {
                    code: None,
                    message: Some(msg),
                    data: (),
                },
            )
            .into()),
        }
    }
}

/// Called by Swift via FFI when transaction updates occur.
fn trigger(event: String, payload: String) -> Result<(), ffi::FFIResult> {
    crate::listeners::trigger(&event, payload)
        .map_err(|e| ffi::FFIResult::Err(format!("Failed to trigger event '{event}': {e}")))
}

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Iap<R>> {
    Ok(Iap {
        _app: app.clone(),
        plugin: ffi::IapPlugin::init_plugin(),
    })
}

/// Access to the iap APIs.
pub struct Iap<R: Runtime> {
    _app: AppHandle<R>,
    plugin: ffi::IapPlugin,
}

impl<R: Runtime> Iap<R> {
    pub async fn get_products(
        &self,
        product_ids: Vec<String>,
        product_type: String,
    ) -> crate::Result<GetProductsResponse> {
        validation::require_bundle()?;

        self.plugin
            .getProducts(product_ids, product_type)
            .await
            .parse()
    }

    pub async fn purchase(&self, payload: PurchaseRequest) -> crate::Result<Purchase> {
        validation::require_bundle()?;

        self.plugin
            .purchase(
                payload.product_id,
                payload.product_type,
                payload.options.and_then(|opts| opts.offer_token),
            )
            .await
            .parse()
    }

    pub async fn restore_purchases(
        &self,
        product_type: String,
    ) -> crate::Result<RestorePurchasesResponse> {
        validation::require_bundle()?;

        self.plugin.restorePurchases(product_type).await.parse()
    }

    pub async fn acknowledge_purchase(
        &self,
        purchase_token: String,
    ) -> crate::Result<AcknowledgePurchaseResponse> {
        validation::require_bundle()?;

        self.plugin
            .acknowledgePurchase(purchase_token)
            .await
            .parse()
    }

    pub async fn get_product_status(
        &self,
        product_id: String,
        product_type: String,
    ) -> crate::Result<ProductStatus> {
        validation::require_bundle()?;

        self.plugin
            .getProductStatus(product_id, product_type)
            .await
            .parse()
    }
}
