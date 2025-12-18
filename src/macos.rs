use serde::de::DeserializeOwned;
use std::sync::{mpsc, OnceLock};
use tauri::{plugin::PluginApi, AppHandle, Emitter, Runtime};

use crate::models::*;

static EVENT_TX: OnceLock<mpsc::Sender<(String, String)>> = OnceLock::new();

mod codesign {
    use objc2_security::{kSecCSCheckAllArchitectures, kSecCSCheckNestedCode, SecCSFlags, SecCode};
    use std::ptr::NonNull;

    /// Returns `Ok(())` if the running binary is code-signed and valid, otherwise returns an Error.
    ///
    /// This validation works for all distribution methods:
    /// - Development builds (signed with development certificate)
    /// - TestFlight builds (signed with TestFlight Beta Distribution certificate)
    /// - App Store builds (signed with App Store distribution certificate)
    ///
    /// Note: We intentionally do not use `kSecCSStrictValidate` as it can cause
    /// validation failures for legitimate App Store and TestFlight builds.
    /// The flags we use still ensure the code signature is valid and intact.
    pub fn is_signature_valid() -> crate::Result<()> {
        unsafe {
            // 1) Get a handle to "self"
            let mut self_code: *mut SecCode = std::ptr::null_mut();
            let self_code_ptr = NonNull::<*mut SecCode>::new_unchecked(&mut self_code);
            let status = SecCode::copy_self(SecCSFlags::empty(), self_code_ptr);
            if status != 0 {
                let error_response = crate::error::ErrorResponse {
                    code: Some(status.to_string()),
                    message: Some(format!("Failed to get code reference: OSStatus {status}")),
                    data: (),
                };
                return Err(crate::error::PluginInvokeError::InvokeRejected(error_response).into());
            }

            // 2) Validate the dynamic code - this checks if the signature is valid
            // Using kSecCSCheckAllArchitectures and kSecCSCheckNestedCode ensures thorough
            // validation without the strict requirements that can fail for App Store/TestFlight builds
            let validity_flags = SecCSFlags(kSecCSCheckAllArchitectures | kSecCSCheckNestedCode);
            let self_code_ref = self_code_ptr.as_ref().as_ref().ok_or_else(|| {
                let error_response = crate::error::ErrorResponse {
                    code: Some("nullCodeRef".to_string()),
                    message: Some("Failed to get code reference: null pointer".to_string()),
                    data: (),
                };
                crate::Error::from(crate::error::PluginInvokeError::InvokeRejected(
                    error_response,
                ))
            })?;
            let status = SecCode::check_validity(self_code_ref, validity_flags, None);
            if status != 0 {
                let error_response = crate::error::ErrorResponse {
                    code: Some(status.to_string()),
                    message: Some(format!(
                        "Code signature validation failed: OSStatus {status}"
                    )),
                    data: (),
                };
                return Err(crate::error::PluginInvokeError::InvokeRejected(error_response).into());
            }

            Ok(())
        }
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
        fn initialize() -> Result<String, FFIResult>;
        async fn getProducts(
            productIds: Vec<String>,
            productType: String,
        ) -> Result<String, FFIResult>;
        async fn purchase(
            productId: String,
            productType: String,
            offerToken: Option<String>,
        ) -> Result<String, FFIResult>;
        async fn restorePurchases(productType: String) -> Result<String, FFIResult>;
        fn acknowledgePurchase(purchaseToken: String) -> Result<String, FFIResult>;
        async fn getProductStatus(
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
    let tx = EVENT_TX
        .get()
        .ok_or_else(|| ffi::FFIResult::Err("Event channel not initialized".to_string()))?;

    tx.send((event, payload))
        .map_err(|e| ffi::FFIResult::Err(format!("Failed to send event: {e}")))
}

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Iap<R>> {
    if EVENT_TX.get().is_none() {
        let (tx, rx) = mpsc::channel();
        let _ = EVENT_TX.set(tx);

        let app_handle = app.clone();
        std::thread::spawn(move || {
            while let Ok((event, payload)) = rx.recv() {
                let _ = app_handle.emit(&event, payload);
            }
        });
    }
    Ok(Iap(app.clone()))
}

/// Access to the iap APIs.
pub struct Iap<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Iap<R> {
    pub fn initialize(&self) -> crate::Result<InitializeResponse> {
        codesign::is_signature_valid()?;

        ffi::initialize().parse()
    }

    pub async fn get_products(
        &self,
        product_ids: Vec<String>,
        product_type: String,
    ) -> crate::Result<GetProductsResponse> {
        codesign::is_signature_valid()?;

        ffi::getProducts(product_ids, product_type).await.parse()
    }

    pub async fn purchase(&self, payload: PurchaseRequest) -> crate::Result<Purchase> {
        codesign::is_signature_valid()?;

        ffi::purchase(
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
        codesign::is_signature_valid()?;

        ffi::restorePurchases(product_type).await.parse()
    }

    pub fn acknowledge_purchase(
        &self,
        purchase_token: String,
    ) -> crate::Result<AcknowledgePurchaseResponse> {
        codesign::is_signature_valid()?;

        ffi::acknowledgePurchase(purchase_token).parse()
    }

    pub async fn get_product_status(
        &self,
        product_id: String,
        product_type: String,
    ) -> crate::Result<ProductStatus> {
        codesign::is_signature_valid()?;

        ffi::getProductStatus(product_id, product_type)
            .await
            .parse()
    }
}
