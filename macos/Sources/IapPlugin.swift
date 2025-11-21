import StoreKit

/// Block the current thread until the async operation finishes.
/// Safe to call from any thread. If on main thread, keeps the run loop alive.
@inline(__always)
func blockOn<T>(_ op: @escaping () async -> T) -> T {
    var out: T?
    let sem = DispatchSemaphore(value: 0)

    Task.detached {
        out = await op()
        sem.signal()
    }

    if Thread.isMainThread {
        // Keep UI responsive while we wait
        while out == nil {
            RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.002))
        }
    } else {
        sem.wait()
    }
    return out!
}

private func serializeToJSON(_ object: [String: Any]) -> String? {
    guard let data = try? JSONSerialization.data(withJSONObject: object),
          let jsonString = String(data: data, encoding: .utf8) else {
        return nil
    }
    return jsonString
}

public func initialize() -> FFIResult {
    // StoreKit 2 doesn't require explicit initialization
    let json: [String: Any] = ["success": true]
    if let jsonString = serializeToJSON(json) {
        return .Ok(RustString(jsonString))
    } else {
        return .Err(RustString("Failed to serialize JSON"))
    }
}

public func getProducts(productIds: RustVec<RustString>, productType: RustString) -> FFIResult {
    blockOn {
        await getProductsAsync(productIds: productIds, productType: productType)
    }
}

@MainActor
func getProductsAsync(productIds: RustVec<RustString>, productType: RustString) async -> FFIResult {
    do {
        let ids: [String] = productIds.map { $0.as_str().toString() }
        let products = try await Product.products(for: ids)
        var productsArray: [[String: Any]] = []

        for product in products {
            var productDict: [String: Any] = [
                "productId": product.id,
                "title": product.displayName,
                "description": product.description,
                "productType": product.type.rawValue
            ]
            
            // Add pricing information
            productDict["formattedPrice"] = product.displayPrice
            productDict["priceCurrencyCode"] = getCurrencyCode(for: product)
            
            // Handle subscription-specific information
            if product.type == .autoRenewable || product.type == .nonRenewable {
                if let subscription = product.subscription {
                    var subscriptionOffers: [[String: Any]] = []
                    
                    // Add introductory offer if available
                    if let introOffer = subscription.introductoryOffer {
                        let offer: [String: Any] = [
                            "offerToken": "",  // macOS doesn't use offer tokens
                            "basePlanId": "",
                            "offerId": introOffer.id ?? "",
                            "pricingPhases": [[
                                "formattedPrice": introOffer.displayPrice,
                                "priceCurrencyCode": getCurrencyCode(for: product),
                                "priceAmountMicros": 0,  // Not available in StoreKit 2
                                "billingPeriod": formatSubscriptionPeriod(introOffer.period),
                                "billingCycleCount": introOffer.periodCount,
                                "recurrenceMode": 0
                            ]]
                        ]
                        subscriptionOffers.append(offer)
                    }
                    
                    // Add regular subscription info
                    let regularOffer: [String: Any] = [
                        "offerToken": "",
                        "basePlanId": "",
                        "offerId": "",
                        "pricingPhases": [[
                            "formattedPrice": product.displayPrice,
                            "priceCurrencyCode": getCurrencyCode(for: product),
                            "priceAmountMicros": 0,
                            "billingPeriod": formatSubscriptionPeriod(subscription.subscriptionPeriod),
                            "billingCycleCount": 0,
                            "recurrenceMode": 1
                        ]]
                    ]
                    subscriptionOffers.append(regularOffer)
                    
                    productDict["subscriptionOfferDetails"] = subscriptionOffers
                }
            } else {
                // One-time purchase
                productDict["priceAmountMicros"] = 0  // Not available in StoreKit 2
            }
            
            productsArray.append(productDict)
        }
        
        let json: [String: Any] = ["products": productsArray]
        if let jsonString = serializeToJSON(json) {
            return .Ok(RustString(jsonString))
        } else {
            return .Err(RustString("Failed to serialize JSON"))
        }
    } catch {
        return .Err(RustString("Failed to fetch products: \(error.localizedDescription)"))
    }
}

public func purchase(productId: RustString, productType: RustString, offerToken: Optional<RustString>) -> FFIResult {
    blockOn {
        await purchaseAsync(productId: productId, productType: productType, offerToken: offerToken)
    }
}

@MainActor
func purchaseAsync(productId: RustString, productType: RustString, offerToken: Optional<RustString>) async -> FFIResult {
    do {
        let id = productId.as_str().toString()
        let products = try await Product.products(for: [id])
        guard let product = products.first else {
            return .Err(RustString("Product not found"))
        }
        
        // Initiate purchase
        let result = try await product.purchase()
        
        switch result {
        case .success(let verification):
            switch verification {
            case .verified(let transaction):
                // Finish the transaction
                await transaction.finish()
                
                let purchase = await createPurchaseObject(from: transaction, product: product)
                if let jsonString = serializeToJSON(purchase) {
                    return .Ok(RustString(jsonString))
                } else {
                    return .Err(RustString("Failed to serialize purchase"))
                }
                
            case .unverified(_, _):
                return .Err(RustString("Transaction verification failed"))
            }
            
        case .userCancelled:
            return .Err(RustString("Purchase cancelled by user"))
            
        case .pending:
            return .Err(RustString("Purchase is pending"))
            
        @unknown default:
            return .Err(RustString("Unknown purchase result"))
        }
    } catch {
        return .Err(RustString("Purchase failed: \(error.localizedDescription)"))
    }
}

public func restorePurchases(productType: RustString) -> FFIResult {
    blockOn {
        await restorePurchasesAsync(productType: productType)
    }
}

@MainActor
func restorePurchasesAsync(productType: RustString) async -> FFIResult {
    var purchases: [[String: Any]] = []
    let requestedType = productType.as_str().toString()
    
    do {
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
                            productTypeMatches = (product.type == .autoRenewable || product.type == .nonRenewable)
                        case "inapp":
                            productTypeMatches = (product.type == .consumable || product.type == .nonConsumable)
                        default:
                            productTypeMatches = true
                        }
                        
                        if productTypeMatches {
                            let purchase = await createPurchaseObject(from: transaction, product: product)
                            purchases.append(purchase)
                        }
                    } else {
                        // No filter, include all
                        let purchase = await createPurchaseObject(from: transaction, product: product)
                        purchases.append(purchase)
                    }
                }
            case .unverified(_, _):
                // Skip unverified transactions
                continue
            }
        }
        
        let json: [String: Any] = ["purchases": purchases]
        if let jsonString = serializeToJSON(json) {
            return .Ok(RustString(jsonString))
        } else {
            return .Err(RustString("Failed to serialize purchases"))
        }
    } catch {
        return .Err(RustString("Failed to restore purchases: \(error.localizedDescription)"))
    }
}

public func acknowledgePurchase(purchaseToken: RustString) -> FFIResult {
    // Not needed on Apple platforms
    let json: [String: Any] = ["success": true]
    if let jsonString = serializeToJSON(json) {
        return .Ok(RustString(jsonString))
    } else {
        return .Err(RustString("Failed to serialize JSON"))
    }
}

public func getProductStatus(productId: RustString, productType: RustString) -> FFIResult {
    blockOn {
        await getProductStatusAsync(productId: productId, productType: productType)
    }
}

@MainActor
func getProductStatusAsync(productId: RustString, productType: RustString) async -> FFIResult {
    let id = productId.as_str().toString()
    
    var statusResult: [String: Any] = [
        "productId": id,
        "isOwned": false
    ]
    
    // Check current entitlements for the specific product
    for await result in Transaction.currentEntitlements {
        switch result {
        case .verified(let transaction):
            if transaction.productID == id {
                statusResult["isOwned"] = true
                statusResult["purchaseTime"] = Int(transaction.purchaseDate.timeIntervalSince1970 * 1000)
                statusResult["purchaseToken"] = String(transaction.id)
                statusResult["isAcknowledged"] = true  // Always true on macOS
                
                // Check if expired/revoked
                if let revocationDate = transaction.revocationDate {
                    statusResult["purchaseState"] = 1  // canceled
                    statusResult["isOwned"] = false
                    statusResult["expirationTime"] = Int(revocationDate.timeIntervalSince1970 * 1000)
                } else if let expirationDate = transaction.expirationDate {
                    if expirationDate < Date() {
                        statusResult["purchaseState"] = 1  // canceled
                        statusResult["isOwned"] = false
                    } else {
                        statusResult["purchaseState"] = 0  // purchased
                    }
                    statusResult["expirationTime"] = Int(expirationDate.timeIntervalSince1970 * 1000)
                } else {
                    statusResult["purchaseState"] = 0  // purchased
                }

                // Check subscription renewal status if it's a subscription
                if let product = try? await Product.products(for: [id]).first {
                    if product.type == .autoRenewable {
                        // Check subscription status
                        if let statuses = try? await product.subscription?.status {
                            for status in statuses {
                                if status.state == .subscribed {
                                    statusResult["isAutoRenewing"] = true
                                } else if status.state == .expired {
                                    statusResult["isAutoRenewing"] = false
                                    statusResult["purchaseState"] = 1  // canceled
                                    statusResult["isOwned"] = false
                                } else if status.state == .inGracePeriod {
                                    statusResult["isAutoRenewing"] = true
                                    statusResult["purchaseState"] = 0  // purchased
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
    
    if let jsonString = serializeToJSON(statusResult) {
        return .Ok(RustString(jsonString))
    } else {
        return .Err(RustString("Failed to serialize status"))
    }
}

// MARK: - Helper Functions

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

private func createPurchaseObject(from transaction: Transaction, product: Product) async -> [String: Any] {
    var isAutoRenewing = false
    
    // Check if it's an auto-renewable subscription
    if product.type == .autoRenewable {
        // Check subscription status
        if let statuses = try? await product.subscription?.status {
            for status in statuses {
                if status.state == .subscribed {
                    isAutoRenewing = true
                    break
                }
            }
        }
    }
    
    return [
        "orderId": String(transaction.id),
        "originalId": String(transaction.originalID),
        "packageName": Bundle.main.bundleIdentifier ?? "",
        "productId": transaction.productID,
        "purchaseTime": Int(transaction.purchaseDate.timeIntervalSince1970 * 1000),
        "purchaseToken": String(transaction.id),
        "purchaseState": transaction.revocationDate == nil ? 0 : 1,  // 0 = purchased, 1 = canceled
        "isAutoRenewing": isAutoRenewing,
        "isAcknowledged": true,  // Always true on macOS
        "originalJson": "",      // Not available in StoreKit 2
        "signature": ""          // Not available in StoreKit 2
    ]
}
