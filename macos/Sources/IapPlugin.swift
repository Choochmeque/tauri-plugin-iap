import StoreKit

extension FFIResult: Error {}

typealias JsonObject = [String: Any]

/// Keep in sync with PurchaseState in guest-js/index.ts
enum PurchaseStateValue: Int {
    case purchased = 0
    case canceled = 1
    case pending = 2
}

class IapPlugin {
    private var updateListenerTask: Task<Void, Error>?

    /// When false, the plugin never calls `Transaction.finish()` itself — the
    /// host app must call `acknowledgePurchase` (or `consumePurchase`) once its
    /// server has validated the receipt, the same contract Google Play imposes
    /// on Android.
    ///
    /// Finishing eagerly is convenient but loses money on failure: a finished
    /// transaction disappears from `Transaction.updates`, so a purchase whose
    /// server-side verification failed leaves the customer charged with no
    /// entitlement and no retry path except Restore.
    private let finishTransactionsAutomatically: Bool

    /// Exposes the setting to the tests without widening it for callers.
    var finishesTransactionsAutomaticallyForTesting: Bool { finishTransactionsAutomatically }

    init(finishTransactionsAutomatically: Bool = true) {
        self.finishTransactionsAutomatically = finishTransactionsAutomatically

        // Start listening for transaction updates
        updateListenerTask = Task {
            for await update in Transaction.updates {
                await self.handleTransactionUpdate(update)
            }
        }
    }

    deinit {
        updateListenerTask?.cancel()
    }

    public func getProducts(productIds: RustVec<RustString>, productType: RustString)
        async throws(FFIResult) -> String
    {
        let ids: [String] = productIds.map { $0.as_str().toString() }
        let products: [Product]
        do {
            products = try await Product.products(for: ids)
        } catch {
            throw FFIResult.Err(
                RustString("Failed to fetch products: \(error.localizedDescription)"))
        }
        var productsArray: [JsonObject] = []

        for product in products {
            var productDict: JsonObject = [
                "productId": product.id,
                "title": product.displayName,
                "description": product.description,
                "productType": product.type.rawValue,
            ]

            // Add pricing information
            productDict["formattedPrice"] = product.displayPrice
            productDict["priceCurrencyCode"] = getCurrencyCode(for: product)

            // Handle subscription-specific information
            if product.type == .autoRenewable || product.type == .nonRenewable {
                if let subscription = product.subscription {
                    var subscriptionOffers: [JsonObject] = []

                    // Add introductory offer if available
                    if let introOffer = subscription.introductoryOffer {
                        let offer: JsonObject = [
                            "offerToken": "",  // macOS doesn't use offer tokens
                            "basePlanId": "",
                            "offerId": introOffer.id ?? "",
                            "pricingPhases": [
                                [
                                    "formattedPrice": introOffer.displayPrice,
                                    "priceCurrencyCode": getCurrencyCode(for: product),
                                    "priceAmountMicros": priceAmountMicros(introOffer.price),
                                    "billingPeriod": formatSubscriptionPeriod(introOffer.period),
                                    "billingCycleCount": introOffer.periodCount,
                                    "recurrenceMode": 0,
                                ]
                            ],
                        ]
                        subscriptionOffers.append(offer)
                    }

                    // Add regular subscription info
                    let regularOffer: JsonObject = [
                        "offerToken": "",
                        "basePlanId": "",
                        "offerId": "",
                        "pricingPhases": [
                            [
                                "formattedPrice": product.displayPrice,
                                "priceCurrencyCode": getCurrencyCode(for: product),
                                "priceAmountMicros": priceAmountMicros(product.price),
                                "billingPeriod": formatSubscriptionPeriod(
                                    subscription.subscriptionPeriod),
                                "billingCycleCount": 0,
                                "recurrenceMode": 1,
                            ]
                        ],
                    ]
                    subscriptionOffers.append(regularOffer)

                    productDict["subscriptionOfferDetails"] = subscriptionOffers
                }
            } else {
                // One-time purchase
                productDict["priceAmountMicros"] = priceAmountMicros(product.price)
            }

            productsArray.append(productDict)
        }

        return try serializeToJSON(["products": productsArray])
    }

    public func purchase(
        productId: RustString, productType: RustString, offerToken: RustString?,
        appAccountToken: RustString?
    )
        async throws(FFIResult) -> String
    {
        let id = productId.as_str().toString()

        // Prepare purchase options.
        //
        // Validated BEFORE the product fetch so a malformed token fails
        // immediately, without a StoreKit round trip, rather than after.
        var purchaseOptions: Set<Product.PurchaseOption> = []

        // Add appAccountToken if provided (must be a valid UUID). StoreKit
        // copies it into the signed transaction as `appAccountToken`, which is
        // what lets a server bind a receipt to the account that bought it —
        // without it a stolen receipt can be redeemed by whoever presents it
        // first. Rejecting a non-UUID here rather than dropping it silently
        // matches iOS: a token that never reaches Apple is a token the server
        // will never see, and a check that quietly stops running is worse than
        // one that fails loudly.
        if let appAccountToken = appAccountToken {
            let token = appAccountToken.as_str().toString()
            guard let uuid = UUID(uuidString: token) else {
                throw FFIResult.Err(
                    RustString("Invalid appAccountToken: must be a valid UUID string"))
            }
            purchaseOptions.insert(.appAccountToken(uuid))
        }

        let products: [Product]
        do {
            products = try await Product.products(for: [id])
        } catch {
            throw FFIResult.Err(
                RustString("Failed to fetch product: \(error.localizedDescription)"))
        }

        guard let product = products.first else {
            throw FFIResult.Err(RustString("Product not found"))
        }

        // Initiate purchase with options
        let result: Product.PurchaseResult
        do {
            result =
                purchaseOptions.isEmpty
                ? try await product.purchase()
                : try await product.purchase(options: purchaseOptions)
        } catch {
            throw FFIResult.Err(RustString("Purchase failed: \(error.localizedDescription)"))
        }

        switch result {
        case .success(let verification):
            switch verification {
            case .verified(let transaction):
                // Only finish here when the host app has not taken over
                // responsibility; see `finishTransactionsAutomatically`.
                if finishTransactionsAutomatically {
                    await transaction.finish()
                }

                let purchase = try await createPurchaseObject(from: verification, product: product)
                return try serializeToJSON(purchase)

            case .unverified(_, _):
                throw FFIResult.Err(RustString("Transaction verification failed"))
            }

        case .userCancelled:
            throw FFIResult.Err(RustString("Purchase cancelled by user"))

        case .pending:
            // NOT an error. `.pending` is Ask to Buy awaiting a parent's
            // approval, or an SCA step-up at the bank — the purchase may still
            // succeed, arriving later through `Transaction.updates`. Reporting
            // it as a failure shows the buyer a red toast for something that is
            // working as designed, so return a purchase in the `pending` state
            // and let the caller decide how to word the wait.
            return try serializeToJSON(pendingPurchaseObject(productId: id))

        @unknown default:
            throw FFIResult.Err(RustString("Unknown purchase result"))
        }
    }

    /// Finishes a transaction the host app has taken responsibility for.
    ///
    /// Looked up in `Transaction.unfinished` rather than an in-memory map so it
    /// still resolves after a relaunch — the case that matters, since an
    /// unfinished transaction is re-delivered on the next launch and that is
    /// precisely when the app retries a verification that failed earlier.
    ///
    /// An unknown token is treated as success: it means the transaction was
    /// already finished, which is the state the caller asked for. Erroring
    /// would make an ordinary retry look like a failure.
    public func finishTransaction(purchaseToken: RustString) async throws(FFIResult) -> String {
        let token = purchaseToken.as_str().toString()

        for await result in Transaction.unfinished {
            guard case .verified(let transaction) = result else { continue }
            if String(transaction.id) == token {
                await transaction.finish()
                break
            }
        }

        return try serializeToJSON(["finished": true])
    }

    public func restorePurchases(productType: RustString) async throws(FFIResult) -> String {
        var purchases: [JsonObject] = []
        let requestedType = productType.as_str().toString()

        // Ask the App Store to re-sync before reading entitlements. On a machine
        // the customer has never launched the app on — or after signing in with
        // a different Apple Account — `currentEntitlements` is empty until this
        // runs, and "Restore Purchases did nothing" is a reliable App Review
        // rejection. Apple requires this be user-initiated, which a Restore
        // button is.
        //
        // A failure here is deliberately not fatal: `sync()` presents an App
        // Store sign-in sheet, and a customer who dismisses it should still get
        // whatever entitlements are already cached locally rather than an error.
        try? await AppStore.sync()

        // Get all current entitlements
        for await result in Transaction.currentEntitlements {
            switch result {
            case .verified(let transaction):
                if let product = try? await Product.products(for: [transaction.productID]).first {
                    // Filter by product type if specified
                    if !requestedType.isEmpty {
                        let productTypeMatches: Bool
                        switch requestedType {
                        case "subs":
                            productTypeMatches =
                                (product.type == .autoRenewable || product.type == .nonRenewable)
                        case "inapp":
                            productTypeMatches =
                                (product.type == .consumable || product.type == .nonConsumable)
                        default:
                            productTypeMatches = true
                        }

                        if productTypeMatches {
                            let purchase = try await createPurchaseObject(from: result, product: product)
                            purchases.append(purchase)
                        }
                    } else {
                        // No filter, include all
                        let purchase = try await createPurchaseObject(from: result, product: product)
                        purchases.append(purchase)
                    }
                }
            case .unverified(_, _):
                // Skip unverified transactions
                continue
            }
        }

        return try serializeToJSON(["purchases": purchases])
    }

    public func getProductStatus(productId: RustString, productType: RustString)
        async throws(FFIResult) -> String
    {
        let id = productId.as_str().toString()

        var statusResult: JsonObject = [
            "productId": id,
            "isOwned": false,
        ]

        // Check current entitlements for the specific product
        for await result in Transaction.currentEntitlements {
            switch result {
            case .verified(let transaction):
                if transaction.productID == id {
                    statusResult["isOwned"] = true
                    statusResult["purchaseTime"] = Int(
                        transaction.purchaseDate.timeIntervalSince1970 * 1000)
                    statusResult["purchaseToken"] = String(transaction.id)
                    statusResult["isAcknowledged"] = true  // Always true on macOS

                    // Check if expired/revoked
                    if let revocationDate = transaction.revocationDate {
                        statusResult["purchaseState"] = PurchaseStateValue.canceled.rawValue
                        statusResult["isOwned"] = false
                        statusResult["expirationTime"] = Int(
                            revocationDate.timeIntervalSince1970 * 1000)
                    } else if let expirationDate = transaction.expirationDate {
                        if expirationDate < Date() {
                            statusResult["purchaseState"] = PurchaseStateValue.canceled.rawValue
                            statusResult["isOwned"] = false
                        } else {
                            statusResult["purchaseState"] = PurchaseStateValue.purchased.rawValue
                        }
                        statusResult["expirationTime"] = Int(
                            expirationDate.timeIntervalSince1970 * 1000)
                    } else {
                        statusResult["purchaseState"] = PurchaseStateValue.purchased.rawValue
                    }

                    // Check subscription renewal status if it's a subscription
                    if let product = try? await Product.products(for: [id]).first {
                        if product.type == .autoRenewable {
                            // Check subscription status
                            if let statuses = try? await product.subscription?.status {
                                for status in statuses {
                                    if status.state == .subscribed {
                                        // `.subscribed` only means the subscription is still active;
                                        // it does NOT imply auto-renew is on. A subscription that the
                                        // user cancelled (but hasn't expired yet) is also `.subscribed`.
                                        // The actual renewal intent lives in renewalInfo.willAutoRenew.
                                        if case .verified(let renewalInfo) = status.renewalInfo {
                                            statusResult["isAutoRenewing"] = renewalInfo.willAutoRenew
                                        } else {
                                            statusResult["isAutoRenewing"] = true
                                        }
                                    } else if status.state == .expired {
                                        statusResult["isAutoRenewing"] = false
                                        statusResult["purchaseState"] =
                                            PurchaseStateValue.canceled.rawValue
                                        statusResult["isOwned"] = false
                                    } else if status.state == .inGracePeriod {
                                        statusResult["isAutoRenewing"] = true
                                        statusResult["purchaseState"] =
                                            PurchaseStateValue.purchased.rawValue
                                    } else {
                                        statusResult["isAutoRenewing"] = false
                                    }
                                    break
                                }
                            }
                        }
                    }

                    break
                }
            case .unverified(_, _):
                // Skip unverified transactions
                continue
            }
        }

        return try serializeToJSON(statusResult)
    }

    // MARK: - Helper Functions

    private func handleTransactionUpdate(_ result: VerificationResult<Transaction>) async {
        switch result {
        case .verified(let transaction):
            // Get product details
            if let product = try? await Product.products(for: [transaction.productID]).first {
                if let purchase = try? await createPurchaseObject(from: result, product: product),
                   let jsonString = try? serializeToJSON(purchase) {
                    try? trigger("purchaseUpdated", jsonString)
                }
            }

            // Finish only when the host app has not taken over. Leaving it
            // unfinished is what makes the retry possible: StoreKit re-delivers
            // it here on the next launch until someone acknowledges it.
            if finishTransactionsAutomatically {
                await transaction.finish()
            }

        case .unverified(_, _):
            // Handle unverified transaction
            break
        }
    }

    /// A `Purchase` for a transaction that does not exist yet.
    ///
    /// `.pending` has no `Transaction` to describe — StoreKit hands one over
    /// later, through `Transaction.updates`, if the purchase is approved. The
    /// shape has to stay in sync with the `Purchase` struct in src/models.rs;
    /// the ids are empty because there is genuinely nothing to identify yet.
    // Not `private` so the tests can assert the shape without a StoreKit daemon.
    func pendingPurchaseObject(productId: String) -> JsonObject {
        return [
            "orderId": "",
            "originalId": "",
            "packageName": Bundle.main.bundleIdentifier ?? "",
            "productId": productId,
            "purchaseTime": 0,
            "purchaseToken": "",
            "purchaseState": PurchaseStateValue.pending.rawValue,
            "isAutoRenewing": false,
            "isAcknowledged": false,
            "originalJson": "",
            "signature": "",
        ]
    }

    private func serializeToJSON(_ object: JsonObject) throws(FFIResult) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: object),
            let jsonString = String(data: data, encoding: .utf8)
        else {
            throw FFIResult.Err(RustString("Failed to serialize JSON"))
        }
        return jsonString
    }

    private func formatSubscriptionPeriod(_ period: Product.SubscriptionPeriod) -> String {
        switch period.unit {
        case .day:
            return "P\(period.value)D"
        case .week:
            return "P\(period.value)W"
        case .month:
            return "P\(period.value)M"
        case .year:
            return "P\(period.value)Y"
        @unknown default:
            return "P1M"
        }
    }

    private func getCurrencyCode(for product: Product) -> String {
        if #available(macOS 13.0, *) {
            return product.priceFormatStyle.locale.currency?.identifier ?? ""
        } else {
            // Fallback for macOS 12: currency code not directly available
            return ""
        }
    }

    private func priceAmountMicros(_ decimal: Decimal) -> Int64 {
        return NSDecimalNumber(decimal: decimal * 1_000_000).int64Value
    }

    private func createPurchaseObject(from verificationResult: VerificationResult<Transaction>, product: Product) async throws(FFIResult)
        -> JsonObject
    {
        guard case .verified(let transaction) = verificationResult else {
            throw FFIResult.Err(RustString("Transaction not verified"))
        }

        var isAutoRenewing = false

        // Check if it's an auto-renewable subscription
        if product.type == .autoRenewable {
            // Check subscription status
            if let statuses = try? await product.subscription?.status {
                for status in statuses {
                    if status.state == .subscribed {
                        // `.subscribed` means the subscription is currently active, but a cancelled
                        // (yet unexpired) subscription also has this state. Use willAutoRenew.
                        if case .verified(let renewalInfo) = status.renewalInfo {
                            isAutoRenewing = renewalInfo.willAutoRenew
                        } else {
                            isAutoRenewing = true
                        }
                        break
                    }
                }
            }
        }

        return [
            "orderId": String(transaction.id),
            "originalId": String(transaction.originalID),
            "jwsRepresentation": verificationResult.jwsRepresentation,
            "packageName": Bundle.main.bundleIdentifier ?? "",
            "productId": transaction.productID,
            "purchaseTime": Int(transaction.purchaseDate.timeIntervalSince1970 * 1000),
            "purchaseToken": String(transaction.id),
            "purchaseState": transaction.revocationDate == nil
                ? PurchaseStateValue.purchased.rawValue : PurchaseStateValue.canceled.rawValue,
            "isAutoRenewing": isAutoRenewing,
            "isAcknowledged": true,  // Always true on macOS
            "originalJson": "",  // Not available in StoreKit 2
            "signature": "",  // Not available in StoreKit 2
        ]
    }
}

// Initialize the plugin
func initPlugin(finishTransactionsAutomatically: Bool = true) -> IapPlugin {
    return IapPlugin(finishTransactionsAutomatically: finishTransactionsAutomatically)
}
