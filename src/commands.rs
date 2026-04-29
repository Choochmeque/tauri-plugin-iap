use tauri::{AppHandle, Runtime, command};

use crate::models::{
    AcknowledgePurchaseRequest, ConsumePurchaseRequest, GetProductStatusRequest,
    GetProductsRequest, GetProductsResponse, InitializeResponse, ProductStatus, Purchase,
    PurchaseRequest, RestorePurchasesRequest, RestorePurchasesResponse,
};
use crate::{IapExt, Result};

#[command]
pub async fn initialize<R: Runtime>(_app: AppHandle<R>) -> Result<InitializeResponse> {
    Err(std::io::Error::other("initialize() is deprecated and no longer needed. The billing client initializes automatically.").into())
}

#[command]
pub async fn get_products<R: Runtime>(
    app: AppHandle<R>,
    payload: GetProductsRequest,
) -> Result<GetProductsResponse> {
    app.iap()
        .get_products(payload.product_ids, payload.product_type)
        .await
}

#[command]
pub async fn purchase<R: Runtime>(app: AppHandle<R>, payload: PurchaseRequest) -> Result<Purchase> {
    app.iap().purchase(payload).await
}

#[command]
pub async fn restore_purchases<R: Runtime>(
    app: AppHandle<R>,
    payload: RestorePurchasesRequest,
) -> Result<RestorePurchasesResponse> {
    app.iap().restore_purchases(payload.product_type).await
}

#[command]
pub async fn acknowledge_purchase<R: Runtime>(
    app: AppHandle<R>,
    payload: AcknowledgePurchaseRequest,
) -> Result<()> {
    app.iap().acknowledge_purchase(payload.purchase_token).await
}

#[command]
pub async fn consume_purchase<R: Runtime>(
    app: AppHandle<R>,
    payload: ConsumePurchaseRequest,
) -> Result<()> {
    app.iap().consume_purchase(payload.purchase_token).await
}

#[command]
pub async fn get_product_status<R: Runtime>(
    app: AppHandle<R>,
    payload: GetProductStatusRequest,
) -> Result<ProductStatus> {
    app.iap()
        .get_product_status(payload.product_id, payload.product_type)
        .await
}
