import {
  invoke,
  addPluginListener,
  PluginListener,
} from "@tauri-apps/api/core";

/**
 * Response from IAP initialization
 */
export interface InitializeResponse {
  success: boolean;
}

/**
 * Represents a pricing phase for subscription products
 */
export interface PricingPhase {
  formattedPrice: string;
  priceCurrencyCode: string;
  priceAmountMicros: number;
  billingPeriod: string;
  billingCycleCount: number;
  recurrenceMode: number;
}

/**
 * Subscription offer details including pricing phases
 */
export interface SubscriptionOffer {
  offerToken: string;
  basePlanId: string;
  offerId?: string;
  pricingPhases: PricingPhase[];
}

/**
 * Product information from the app store
 */
export interface Product {
  /** Unique product identifier as configured in the app store */
  productId: string;
  /** Localized product title */
  title: string;
  /** Localized product description */
  description: string;
  /** Type of product: "subs" for subscriptions, "inapp" for one-time purchases */
  productType: string;
  /** Localized price string with currency symbol (e.g., "$9.99") */
  formattedPrice?: string;
  /** ISO 4217 currency code (e.g., "USD", "EUR") */
  priceCurrencyCode?: string;
  /** Price in micros (price × 1,000,000). For example, $9.99 = 9990000 */
  priceAmountMicros?: number;
  /** Subscription offer details including pricing phases. (Android only) */
  subscriptionOfferDetails?: SubscriptionOffer[];
}

/**
 * Response containing products fetched from the store
 */
export interface GetProductsResponse {
  products: Product[];
}

/**
 * Purchase transaction information
 */
export interface Purchase {
  /** Unique order identifier from the store. May be undefined for pending purchases. */
  orderId?: string;
  /** Application package name (Android) or bundle identifier (iOS/macOS) */
  packageName: string;
  /** Product identifier that was purchased */
  productId: string;
  /** Unix timestamp (milliseconds) when the purchase was made */
  purchaseTime: number;
  /** Token used to identify this purchase for acknowledgment and server-side verification */
  purchaseToken: string;
  /** Current state of the purchase. */
  purchaseState: PurchaseState;
  /** Whether this subscription is set to auto-renew. Always false for one-time purchases. */
  isAutoRenewing: boolean;
  /** Whether the purchase has been acknowledged. Unacknowledged purchases are refunded after 3 days. (Android only, always true on iOS/macOS) */
  isAcknowledged: boolean;
  /** Raw JSON response from the store for server-side verification. (Android only) */
  originalJson: string;
  /** Cryptographic signature for purchase verification. (Android only) */
  signature: string;
  /** Original transaction ID. Used to link renewals and restores to the original purchase. (iOS/macOS only) */
  originalId?: string;
  /** JWS representation of the signed transaction for server-side validation. (iOS/macOS only) */
  jwsRepresentation?: string;
}

/**
 * Response containing restored purchases
 */
export interface RestorePurchasesResponse {
  purchases: Purchase[];
}

/**
 * Historical purchase record
 */
export interface PurchaseHistoryRecord {
  productId: string;
  purchaseTime: number;
  purchaseToken: string;
  quantity: number;
  originalJson: string;
  signature: string;
}

/**
 * Response containing purchase history
 */
export interface GetPurchaseHistoryResponse {
  history: PurchaseHistoryRecord[];
}

/**
 * Response from acknowledging a purchase
 */
export interface AcknowledgePurchaseResponse {
  success: boolean;
}

/**
 * Purchase state enumeration
 */
export enum PurchaseState {
  PURCHASED = 0,
  CANCELED = 1,
  PENDING = 2,
}

/**
 * Current status of a product for the user
 */
export interface ProductStatus {
  productId: string;
  isOwned: boolean;
  purchaseState?: PurchaseState;
  purchaseTime?: number;
  expirationTime?: number;
  isAutoRenewing?: boolean;
  isAcknowledged?: boolean;
  purchaseToken?: string;
}

/**
 * Google Play subscription replacement modes for upgrades/downgrades.
 * Used with `subscriptionReplacementMode` in `PurchaseOptions`.
 * @see https://developer.android.com/reference/com/android/billingclient/api/BillingFlowParams.SubscriptionUpdateParams.ReplacementMode
 */
export enum SubscriptionReplacementMode {
  /** Replacement takes effect when the old plan expires, and the new price is charged at the same time. */
  DEFERRED = 6,
  /** Replacement takes effect immediately. The billing cycle remains the same. The remaining value from the old price is prorated for the new plan. */
  WITH_TIME_PRORATION = 1,
  /** Replacement takes effect immediately. The new price is charged immediately and in full. Any remaining period from the old plan is used to extend the new billing date. */
  CHARGE_FULL_PRICE = 5,
  /** Replacement takes effect immediately. The new plan price is reduced by the prorated cost of the old plan for the remaining period. */
  CHARGE_PRORATED_PRICE = 2,
  /** Replacement takes effect immediately with no proration. The user is charged full price for the new plan. */
  WITHOUT_PRORATION = 3,
}

/**
 * Optional parameters for purchase requests
 */
export interface PurchaseOptions {
  /** Offer token for subscription products (Android) */
  offerToken?: string;
  /** Obfuscated account identifier for fraud prevention (Android only) */
  obfuscatedAccountId?: string;
  /** Obfuscated profile identifier for fraud prevention (Android only) */
  obfuscatedProfileId?: string;
  /** App account token - must be a valid UUID string (iOS only) */
  appAccountToken?: string;
  /**
   * Purchase token of the existing subscription to replace (Android only).
   * When set, the purchase becomes a subscription upgrade/downgrade.
   * Obtain this from a previous purchase's `purchaseToken` field.
   */
  oldPurchaseToken?: string;
  /**
   * Replacement mode for subscription upgrades/downgrades (Android only).
   * Determines how the transition between old and new subscription is handled.
   * Use values from the `SubscriptionReplacementMode` enum.
   * Defaults to `WITH_TIME_PRORATION` if not specified.
   * @see SubscriptionReplacementMode
   */
  subscriptionReplacementMode?: SubscriptionReplacementMode;
}

/**
 * Initialize the IAP plugin.
 *
 * @deprecated This function is no longer needed. The billing client is now initialized automatically when the plugin loads. This function will be removed in the next major release.
 * @returns Promise resolving to `{ success: true }` for backward compatibility
 */
export async function initialize(): Promise<InitializeResponse> {
  return await invoke<InitializeResponse>("plugin:iap|initialize");
}

/**
 * Fetch product information from the app store.
 *
 * @param productIds - Array of product identifiers to fetch
 * @param productType - Type of products: "subs" for subscriptions, "inapp" for one-time purchases
 * @returns Promise resolving to product information
 * @example
 * ```typescript
 * const { products } = await getProducts(
 *   ['com.example.premium', 'com.example.remove_ads'],
 *   'inapp'
 * );
 * ```
 */
export async function getProducts(
  productIds: string[],
  productType: "subs" | "inapp" = "subs",
): Promise<GetProductsResponse> {
  return await invoke<GetProductsResponse>("plugin:iap|get_products", {
    payload: {
      productIds,
      productType,
    },
  });
}

/**
 * Initiate a purchase for the specified product.
 *
 * @param productId - Product identifier to purchase
 * @param productType - Type of product: "subs" or "inapp"
 * @param options - Optional purchase parameters (platform-specific)
 * @returns Promise resolving to purchase transaction details
 * @example
 * ```typescript
 * // Simple purchase
 * const purchase = await purchase('com.example.premium', 'subs');
 *
 * // With options (iOS)
 * const purchase = await purchase('com.example.premium', 'subs', {
 *   appAccountToken: '550e8400-e29b-41d4-a716-446655440000' // Must be valid UUID
 * });
 *
 * // With options (Android)
 * const purchase = await purchase('com.example.premium', 'subs', {
 *   offerToken: 'offer_token_here',
 *   obfuscatedAccountId: 'user_account_id',
 *   obfuscatedProfileId: 'user_profile_id'
 * });
 *
 * // Subscription upgrade/downgrade (Android)
 * const purchase = await purchase('com.example.premium', 'subs', {
 *   offerToken: 'new_plan_offer_token',
 *   oldPurchaseToken: 'existing_subscription_purchase_token',
 *   subscriptionReplacementMode: SubscriptionReplacementMode.WITH_TIME_PRORATION
 * });
 * ```
 */
export async function purchase(
  productId: string,
  productType: "subs" | "inapp" = "subs",
  options?: PurchaseOptions,
): Promise<Purchase> {
  return await invoke<Purchase>("plugin:iap|purchase", {
    payload: {
      productId,
      productType,
      ...options,
    },
  });
}

/**
 * Restore user's previous purchases.
 *
 * @param productType - Type of products to restore: "subs" or "inapp"
 * @returns Promise resolving to list of restored purchases
 * @example
 * ```typescript
 * const { purchases } = await restorePurchases('subs');
 * purchases.forEach(purchase => {
 *   console.log(`Restored: ${purchase.productId}`);
 * });
 * ```
 */
export async function restorePurchases(
  productType: "subs" | "inapp" = "subs",
): Promise<RestorePurchasesResponse> {
  return await invoke<RestorePurchasesResponse>(
    "plugin:iap|restore_purchases",
    {
      payload: {
        productType,
      },
    },
  );
}

/**
 * Get the user's purchase history.
 * Note: Not supported on all platforms.
 *
 * @returns Promise resolving to purchase history
 * @example
 * ```typescript
 * const { history } = await getPurchaseHistory();
 * history.forEach(record => {
 *   console.log(`Purchase: ${record.productId} at ${record.purchaseTime}`);
 * });
 * ```
 */
export async function getPurchaseHistory(): Promise<GetPurchaseHistoryResponse> {
  return await invoke<GetPurchaseHistoryResponse>(
    "plugin:iap|get_purchase_history",
  );
}

/**
 * Acknowledge a purchase (Android only).
 * Purchases must be acknowledged within 3 days or they will be refunded.
 * iOS automatically acknowledges purchases.
 *
 * @param purchaseToken - Purchase token from the transaction
 * @returns Promise resolving to acknowledgment status
 * @example
 * ```typescript
 * const result = await acknowledgePurchase(purchase.purchaseToken);
 * if (result.success) {
 *   console.log('Purchase acknowledged');
 * }
 * ```
 */
export async function acknowledgePurchase(
  purchaseToken: string,
): Promise<AcknowledgePurchaseResponse> {
  return await invoke<AcknowledgePurchaseResponse>(
    "plugin:iap|acknowledge_purchase",
    {
      payload: {
        purchaseToken,
      },
    },
  );
}

/**
 * Get the current status of a product for the user.
 * Checks if the product is owned, expired, or available for purchase.
 *
 * @param productId - Product identifier to check
 * @param productType - Type of product: "subs" or "inapp"
 * @returns Promise resolving to product status
 * @example
 * ```typescript
 * const status = await getProductStatus('com.example.premium', 'subs');
 * if (status.isOwned) {
 *   console.log('User owns this product');
 *   if (status.isAutoRenewing) {
 *     console.log('Subscription is auto-renewing');
 *   }
 * }
 * ```
 */
export async function getProductStatus(
  productId: string,
  productType: "subs" | "inapp" = "subs",
): Promise<ProductStatus> {
  return await invoke<ProductStatus>("plugin:iap|get_product_status", {
    payload: {
      productId,
      productType,
    },
  });
}

/**
 * Listen for purchase updates.
 * This event is triggered when a purchase state changes.
 *
 * @param callback - Function to call when a purchase is updated
 * @returns Promise resolving to a PluginListener that can be used to stop listening
 * @example
 * ```typescript
 * const listener = await onPurchaseUpdated((purchase) => {
 *   console.log(`Purchase updated: ${purchase.productId}`);
 *   if (purchase.purchaseState === PurchaseState.PURCHASED) {
 *     // Handle successful purchase
 *   }
 * });
 *
 * // Later, stop listening
 * await listener.unregister();
 * ```
 */
export async function onPurchaseUpdated(
  callback: (purchase: Purchase) => void,
): Promise<PluginListener> {
  return await addPluginListener("iap", "purchaseUpdated", callback);
}
