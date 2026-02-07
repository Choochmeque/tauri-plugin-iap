[![NPM Version](https://img.shields.io/npm/v/@choochmeque%2Ftauri-plugin-iap-api)](https://www.npmjs.com/package/@choochmeque/tauri-plugin-iap-api)
[![Crates.io Version](https://img.shields.io/crates/v/tauri-plugin-iap)](https://crates.io/crates/tauri-plugin-iap)
[![Tests](https://github.com/Choochmeque/tauri-plugin-iap/actions/workflows/tests.yml/badge.svg)](https://github.com/Choochmeque/tauri-plugin-iap/actions/workflows/tests.yml)
[![codecov](https://codecov.io/gh/Choochmeque/tauri-plugin-iap/branch/main/graph/badge.svg)](https://codecov.io/gh/Choochmeque/tauri-plugin-iap)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

# Tauri Plugin IAP

A Tauri plugin for In-App Purchases (IAP) with support for subscriptions on iOS (StoreKit 2), Android (Google Play Billing), Windows (Microsoft Store) and macOS (StoreKit 2).

## Features

- Query products and subscriptions with detailed pricing
- Purchase subscriptions with platform-specific features
- Restore previous purchases
- Get purchase history
- Check product ownership and subscription status
- Real-time purchase state updates via events
- Automatic transaction verification (iOS)
- Support for introductory offers and free trials
- Fraud prevention with obfuscated account/profile IDs (Android)
- App account token support for tracking (iOS)
- Automatic offer token selection (Android)
- Subscription upgrades/downgrades with proration modes (Android)

## Platform Support

- **iOS**: StoreKit 2 (requires iOS 15.0+)
- **Android**: Google Play Billing Library v8.0.0
- **Windows**: Microsoft Store API (Windows 10/11)
- **macOS**: StoreKit 2 (requires macOS 13.0+)

## Installation

Install the JavaScript package:

```bash
npm install @choochmeque/tauri-plugin-iap-api
# or
yarn add @choochmeque/tauri-plugin-iap-api
# or
pnpm add @choochmeque/tauri-plugin-iap-api
```

Add the plugin to your Tauri project's `Cargo.toml`:

```toml
[dependencies]
tauri-plugin-iap = "0.7"
```

Configure the plugin permissions in your `capabilities/default.json`:

```json
{
  "permissions": [
    "iap:default"
  ]
}
```

Register the plugin in your Tauri app:

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_iap::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## Example App

An example application is available in the [`examples/iap-demo`](examples/iap-demo) directory. The example demonstrates all core IAP functionality with a UI:

- Fetch and display products with pricing
- Purchase products and subscriptions
- Restore previous purchases
- Check product ownership status
- Real-time purchase update notifications

### Running the Example

```bash
cd examples/iap-demo

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build for production
pnpm tauri build
```

**Important:** The app must be properly code-signed to test IAP functionality:
- **iOS/macOS**: Code signing is required by StoreKit. Configure signing in Xcode or via Tauri's configuration.
- **Android**: Sign your APK/AAB with a release keystore for production testing.
- **Windows**: App must be associated with Microsoft Store and signed for Store submission.

The example app provides a fully functional demo with inline documentation for each IAP feature, making it easy to understand how to integrate the plugin into your own application.

## Usage

### JavaScript/TypeScript

```typescript
import {
  getProducts,
  purchase,
  restorePurchases,
  acknowledgePurchase,
  getProductStatus,
  onPurchaseUpdated,
  PurchaseState
} from '@choochmeque/tauri-plugin-iap-api';

// Get available products
const products = await getProducts(['subscription_id_1', 'subscription_id_2'], 'subs');

// Check if user owns a specific product
const status = await getProductStatus('subscription_id_1', 'subs');
if (status.isOwned && status.purchaseState === PurchaseState.PURCHASED) {
  console.log('User has active subscription');
  if (status.isAutoRenewing) {
    console.log('Subscription will auto-renew');
  }
}

// Purchase a subscription or in-app product
// Simple purchase (will use first available offer on Android if not specified)
const purchaseResult = await purchase('subscription_id_1', 'subs');

// With specific offer token (Android)
const purchaseResult = await purchase('subscription_id_1', 'subs', {
  offerToken: product.subscriptionOfferDetails[0].offerToken
});

// With fraud prevention (Android)
const purchaseResult = await purchase('subscription_id_1', 'subs', {
  obfuscatedAccountId: 'hashed_account_id',
  obfuscatedProfileId: 'hashed_profile_id'
});

// With app account token (iOS - must be valid UUID)
const purchaseResult = await purchase('subscription_id_1', 'subs', {
  appAccountToken: '550e8400-e29b-41d4-a716-446655440000'
});

// Upgrade/downgrade subscription (Android)
import { SubscriptionReplacementMode } from '@choochmeque/tauri-plugin-iap-api';

const upgraded = await purchase('premium_subscription', 'subs', {
  offerToken: 'premium_offer_token',
  oldPurchaseToken: currentSubscription.purchaseToken,
  subscriptionReplacementMode: SubscriptionReplacementMode.WITH_TIME_PRORATION
});

// Restore purchases (specify product type)
const restored = await restorePurchases('subs');

// Acknowledge a purchase (Android only, iOS auto-acknowledges)
await acknowledgePurchase(purchaseResult.purchaseToken);

// Listen for purchase updates
const listener = await onPurchaseUpdated((purchase) => {
  console.log('Purchase updated:', purchase);
});

// Stop listening
await listener.unregister();
```

## Platform Setup

### iOS Setup

1. Configure your app in App Store Connect
2. Create subscription products with appropriate pricing
3. Add In-App Purchase capability to your app in Xcode:
   - Open your project in Xcode
   - Select your target
   - Go to "Signing & Capabilities"
   - Click "+" and add "In-App Purchase"
4. Test with sandbox accounts

### Android Setup

1. Add your app to Google Play Console
2. Create subscription products in Google Play Console
3. Configure your app's billing permissions (already included in the plugin)
4. Test with test accounts or sandbox environment

### Windows Setup

1. Register your app in Microsoft Partner Center
2. Create add-on products (consumables, durables, or subscriptions)
3. Associate your app with the Microsoft Store
4. Test with Windows sandbox environment

### macOS Setup

1. Configure your app in App Store Connect
2. Create subscription or in-app purchase products with pricing
3. Configure code signing in `tauri.conf.json`:
   ```json
   {
     "bundle": {
       "macOS": {
         "signingIdentity": "Developer ID Application: Your Name (TEAM_ID)",
         "entitlements": "path/to/entitlements.plist"
       }
     }
   }
   ```
4. Add required entitlements for StoreKit (create `entitlements.plist`):
   ```xml
   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0">
   <dict>
       <key>com.apple.security.app-sandbox</key>
       <true/>
       <key>com.apple.security.network.client</key>
       <true/>
   </dict>
   </plist>
   ```
5. Test with sandbox accounts or StoreKit Configuration files
6. **Important**: App must be code-signed to use StoreKit APIs

## API Reference

### `initialize()` *(Deprecated)*
> **Deprecated**: This function is no longer needed and will be removed in a future major release. The billing client is now initialized automatically when the plugin loads.

Returns `{ success: true }` for backward compatibility.

### `getProducts(productIds: string[], productType: 'subs' | 'inapp')`
Fetches product details from the store.

**Returns:**
- `products`: Array of product objects with:
  - `productId`: Product identifier
  - `title`: Display name
  - `description`: Product description
  - `productType`: Type of product
  - `formattedPrice`: Localized price string
  - `subscriptionOfferDetails`: (subscriptions only) Array of offers

### `purchase(productId: string, productType: 'subs' | 'inapp' = 'subs', options?: PurchaseOptions)`
Initiates a purchase flow with enhanced options for fraud prevention and account management.

**Parameters:**
- `productId`: The product to purchase
- `productType`: Type of product ('subs' for subscriptions, 'inapp' for one-time purchases), defaults to 'subs'
- `options`: Optional purchase parameters:
  - `offerToken`: (Android) Specific offer to purchase. If not provided, uses first available offer
  - `obfuscatedAccountId`: (Android) Hashed account ID for fraud prevention
  - `obfuscatedProfileId`: (Android) Hashed profile ID for fraud prevention
  - `appAccountToken`: (iOS) UUID string for account tracking and fraud prevention
  - `oldPurchaseToken`: (Android) Purchase token of existing subscription to replace for upgrades/downgrades
  - `subscriptionReplacementMode`: (Android) Proration mode using `SubscriptionReplacementMode` enum (defaults to `WITH_TIME_PRORATION`)

**Returns:** Purchase object with transaction details

### `restorePurchases(productType: 'subs' | 'inapp' = 'subs')`
Queries and returns all active purchases.

**Parameters:**
- `productType`: Type of products to restore ('subs' or 'inapp'), defaults to 'subs'

### `getPurchaseHistory()`
Returns the complete purchase history.

### `acknowledgePurchase(purchaseToken: string)`
Acknowledges a purchase (required on Android within 3 days, no-op on iOS).

### `getProductStatus(productId: string, productType: 'subs' | 'inapp' = 'subs')`
Checks the ownership and subscription status of a specific product.

**Parameters:**
- `productId`: The product identifier to check
- `productType`: Type of product ('subs' or 'inapp'), defaults to 'subs'

**Returns:** ProductStatus object with:
- `productId`: Product identifier
- `isOwned`: Whether the user currently owns the product
- `purchaseState`: Current state (PURCHASED=0, CANCELED=1, PENDING=2)
- `purchaseTime`: When the product was purchased (timestamp)
- `expirationTime`: (subscriptions only) When the subscription expires
- `isAutoRenewing`: (subscriptions only) Whether auto-renewal is enabled
- `isAcknowledged`: Whether the purchase has been acknowledged
- `purchaseToken`: Token for the purchase transaction

### `onPurchaseUpdated(callback: (purchase: Purchase) => void): Promise<PluginListener>`
Listens for purchase state changes.

**Returns:** A `PluginListener` object with an `unregister()` method to stop listening.

## Differences Between Platforms

### iOS (StoreKit 2)
- Automatic transaction verification
- No manual acknowledgment needed
- Supports introductory offers and promotional offers
- Transaction updates are automatically observed
- Requires iOS 15.0+

### Android (Google Play Billing)
- Manual acknowledgment required within 3 days
- Supports multiple subscription offers per product
- Offer tokens required for subscription purchases
- More detailed pricing phase information

### Windows (Microsoft Store)
- Automatic acknowledgment handled by the Store
- Supports consumables, durables, and subscriptions
- Uses SKUs for subscription offer variations

### macOS (StoreKit 2)
- Same StoreKit 2 API as iOS
- Automatic transaction verification
- No manual acknowledgment needed
- Requires macOS 13.0+
- App must be code-signed (StoreKit requires valid signature)

## Testing

### iOS
1. Use sandbox test accounts
2. Test on physical devices (subscriptions don't work well on simulators)
3. Clear purchase history in Settings > App Store > Sandbox Account

### Android
1. Upload your app to internal testing track
2. Add test accounts in Google Play Console
3. Test with test payment methods

### Windows
1. Use Microsoft Store sandbox environment
2. Configure test accounts in Partner Center
3. Test with Windows Dev Center test payment methods
4. Ensure app is associated with Store listing

### macOS
1. Use sandbox test accounts (same as iOS)
2. Use StoreKit Configuration files for local testing
3. App must be code-signed to use StoreKit
4. Clear purchase history in System Settings > App Store > Sandbox Account

## License

[MIT](LICENSE)
