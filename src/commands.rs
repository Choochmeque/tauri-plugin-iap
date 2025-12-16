use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::{IapExt, Result};

#[command]
pub(crate) async fn initialize<R: Runtime>(app: AppHandle<R>) -> Result<InitializeResponse> {
    app.iap().initialize()
}

#[command]
pub(crate) async fn get_products<R: Runtime>(
    app: AppHandle<R>,
    payload: GetProductsRequest,
) -> Result<GetProductsResponse> {
    app.iap()
        .get_products(payload.product_ids, payload.product_type).await
}

#[command]
pub(crate) async fn purchase<R: Runtime>(
    app: AppHandle<R>,
    payload: PurchaseRequest,
) -> Result<Purchase> {
    app.iap().purchase(payload)
}

#[command]
pub(crate) async fn restore_purchases<R: Runtime>(
    app: AppHandle<R>,
    payload: RestorePurchasesRequest,
) -> Result<RestorePurchasesResponse> {
    app.iap().restore_purchases(payload.product_type)
}

#[command]
pub(crate) async fn acknowledge_purchase<R: Runtime>(
    app: AppHandle<R>,
    payload: AcknowledgePurchaseRequest,
) -> Result<AcknowledgePurchaseResponse> {
    app.iap().acknowledge_purchase(payload.purchase_token)
}

#[command]
pub(crate) async fn get_product_status<R: Runtime>(
    app: AppHandle<R>,
    payload: GetProductStatusRequest,
) -> Result<ProductStatus> {
    app.iap()
        .get_product_status(payload.product_id, payload.product_type)
}
