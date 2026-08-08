use serde::de::DeserializeOwned;
use tauri::{AppHandle, Runtime, plugin::PluginApi};

use crate::models::{
    GetProductsResponse, ProductStatus, Purchase, PurchaseRequest, RestorePurchasesRequest,
    RestorePurchasesResponse,
};

/// Validation checks for macOS IAP functionality.
///
/// `StoreKit` requires the app to run from a signed `.app` bundle to communicate
/// with the App Store. During development with `tauri dev`, the binary runs
/// directly without a bundle, causing `StoreKit` calls to fail silently or crash.
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
        fn init_plugin(finishTransactionsAutomatically: bool) -> IapPlugin;

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
            appAccountToken: Option<String>,
        ) -> Result<String, FFIResult>;
        async fn restorePurchases(&self, productType: String) -> Result<String, FFIResult>;
        async fn finishTransaction(&self, purchaseToken: String) -> Result<String, FFIResult>;
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

// Signature matches the swift-bridge `extern "Rust"` declaration above, which
// requires `String` (bridge ABI) — `&str` would change the FFI binding.
/// Called by Swift via FFI when transaction updates occur.
#[allow(clippy::needless_pass_by_value)]
fn trigger(event: String, payload: String) -> Result<(), ffi::FFIResult> {
    crate::listeners::trigger(&event, &payload)
        .map_err(|e| ffi::FFIResult::Err(format!("Failed to trigger event '{event}': {e}")))
}

// `Result` matches the cross-platform `init` signature (mobile genuinely fails);
// macOS body is infallible today but the contract is shared.
#[allow(clippy::unnecessary_wraps)]
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: &PluginApi<R, C>,
    config: crate::Config,
) -> crate::Result<Iap<R>> {
    Ok(Iap {
        _app: app.clone(),
        plugin: ffi::IapPlugin::init_plugin(config.finish_transactions_automatically),
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

        // Destructured rather than `and_then`-ed twice: `options` is an
        // `Option<PurchaseOptions>` and the first `and_then` would move it.
        let (offer_token, app_account_token) = payload
            .options
            .map_or((None, None), |opts| (opts.offer_token, opts.app_account_token));

        self.plugin
            .purchase(
                payload.product_id,
                payload.product_type,
                offer_token,
                app_account_token,
            )
            .await
            .parse()
    }

    pub async fn restore_purchases(
        &self,
        request: RestorePurchasesRequest,
    ) -> crate::Result<RestorePurchasesResponse> {
        validation::require_bundle()?;

        // The Microsoft-only fields on `request` are ignored here;
        // macOS gets only the cross-platform `product_type`.
        self.plugin
            .restorePurchases(request.product_type)
            .await
            .parse()
    }

    /// Finishes the transaction, telling `StoreKit` the entitlement has been
    /// delivered and it can stop re-delivering it.
    ///
    /// With the default `finish_transactions_automatically(true)` the plugin has
    /// already finished it, so this is a no-op — the Swift side treats an
    /// unknown or already-finished token as success, which keeps the call
    /// idempotent and safe to retry.
    pub async fn acknowledge_purchase(&self, purchase_token: String) -> crate::Result<()> {
        validation::require_bundle()?;

        self.plugin
            .finishTransaction(purchase_token)
            .await
            .parse::<serde::de::IgnoredAny>()
            .map(|_| ())
    }

    /// Same as [`Self::acknowledge_purchase`]: for a consumable, finishing the
    /// transaction is exactly what lets `StoreKit` sell it again.
    pub async fn consume_purchase(&self, purchase_token: String) -> crate::Result<()> {
        validation::require_bundle()?;

        self.plugin
            .finishTransaction(purchase_token)
            .await
            .parse::<serde::de::IgnoredAny>()
            .map(|_| ())
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
