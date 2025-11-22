import Tauri
import UIKit
import WebKit
import StoreKit

class InitializeArgs: Decodable {}

class GetProductsArgs: Decodable {
    let productIds: [String]
    let productType: String
}

class PurchaseArgs: Decodable {
    let productId: String
    let productType: String?
    let offerToken: String?
    let appAccountToken: String?
}

class RestorePurchasesArgs: Decodable {
    let productType: String?
}

class GetPurchaseHistoryArgs: Decodable {}

class AcknowledgePurchaseArgs: Decodable {
    let purchaseToken: String
}

class GetProductStatusArgs: Decodable {
    let productId: String
    let productType: String?
}

enum PurchaseStateValue: Int {
    case purchased = 0
    case canceled = 1
    case pending = 2
}

@available(iOS 15.0, *)
class IapPlugin: Plugin {
    private var updateListenerTask: Task<Void, Error>?
    
    public override func load(webview: WKWebView) {
        super.load(webview: webview)

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
    
    @objc public func initialize(_ invoke: Invoke) throws {
        // StoreKit 2 doesn't require explicit initialization
        invoke.resolve(["success": true])
    }
    
    @objc public func getProducts(_ invoke: Invoke) async throws {
        let args = try invoke.parseArgs(GetProductsArgs.self)

        do {
            let products = try await Product.products(for: args.productIds)
            var productsArray: [JsonObject] = []

            for product in products {
                var productDict: JsonObject = [
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
                        var subscriptionOffers: [JsonObject] = []

                        // Add introductory offer if available
                        if let introOffer = subscription.introductoryOffer {
                            let offer: JsonObject = [
                                "offerToken": "",  // iOS doesn't use offer tokens
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
                        let regularOffer: JsonObject = [
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
            
            invoke.resolve(["products": productsArray])
        } catch {
            invoke.reject("Failed to fetch products: \(error.localizedDescription)")
        }
    }
    
    @objc public func purchase(_ invoke: Invoke) async throws {
        let args = try invoke.parseArgs(PurchaseArgs.self)
        
        do {
            let products = try await Product.products(for: [args.productId])
            guard let product = products.first else {
                invoke.reject("Product not found")
                return
            }
            
            // Prepare purchase options
            var purchaseOptions: Set<Product.PurchaseOption> = []
            
            // Add appAccountToken if provided (must be a valid UUID)
            if let appAccountToken = args.appAccountToken {
                guard let uuid = UUID(uuidString: appAccountToken) else {
                    invoke.reject("Invalid appAccountToken: must be a valid UUID string")
                    return
                }
                purchaseOptions.insert(.appAccountToken(uuid))
            }
            
            // Initiate purchase with options
            let result = purchaseOptions.isEmpty 
                ? try await product.purchase()
                : try await product.purchase(options: purchaseOptions)
            
            switch result {
            case .success(let verification):
                switch verification {
                case .verified(let transaction):
                    // Finish the transaction
                    await transaction.finish()
                    
                    let purchase = await createPurchaseObject(from: transaction, product: product)
                    invoke.resolve(purchase)
                    
                case .unverified(_, _):
                    invoke.reject("Transaction verification failed")
                }
                
            case .userCancelled:
                invoke.reject("Purchase cancelled by user")
                
            case .pending:
                invoke.reject("Purchase is pending")
                
            @unknown default:
                invoke.reject("Unknown purchase result")
            }
        } catch {
            invoke.reject("Purchase failed: \(error.localizedDescription)")
        }
    }
    
    @objc public func restorePurchases(_ invoke: Invoke) async throws {
        let args = try? invoke.parseArgs(RestorePurchasesArgs.self)
        var purchases: [JsonObject] = []
        
        do {
            // Get all current entitlements
            for await result in Transaction.currentEntitlements {
                switch result {
                case .verified(let transaction):
                    if let product = try? await Product.products(for: [transaction.productID]).first {
                        // Filter by product type if specified
                        if let requestedType = args?.productType {
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
            
            invoke.resolve(["purchases": purchases])
        } catch {
            invoke.reject("Failed to restore purchases: \(error.localizedDescription)")
        }
    }

    @objc public func getPurchaseHistory(_ invoke: Invoke) async throws {
        var history: [JsonObject] = []
        
        do {
            // Get all transactions (including expired ones)
            for await result in Transaction.all {
                switch result {
                case .verified(let transaction):
                    let record: JsonObject = [
                        "productId": transaction.productID,
                        "purchaseTime": Int(transaction.purchaseDate.timeIntervalSince1970 * 1000),
                        "purchaseToken": String(transaction.id),
                        "quantity": transaction.purchasedQuantity,
                        "originalJson": "",  // Not available in StoreKit 2
                        "signature": ""      // Not available in StoreKit 2
                    ]
                    history.append(record)
                case .unverified(_, _):
                    continue
                }
            }
            
            invoke.resolve(["history": history])
        } catch {
            invoke.reject("Failed to get purchase history: \(error.localizedDescription)")
        }
    }
    
    @objc public func acknowledgePurchase(_ invoke: Invoke) throws {
        // iOS automatically acknowledges purchases, so this is a no-op
        invoke.resolve(["success": true])
    }
    
    @objc public func getProductStatus(_ invoke: Invoke) async throws {
        let args = try invoke.parseArgs(GetProductStatusArgs.self)

        var statusResult: JsonObject = [
            "productId": args.productId,
            "isOwned": false
        ]
        
        // Check current entitlements for the specific product
        for await result in Transaction.currentEntitlements {
            switch result {
            case .verified(let transaction):
                if transaction.productID == args.productId {
                    statusResult["isOwned"] = true
                    statusResult["purchaseTime"] = Int(transaction.purchaseDate.timeIntervalSince1970 * 1000)
                    statusResult["purchaseToken"] = String(transaction.id)
                    statusResult["isAcknowledged"] = true  // Always true on iOS
                    
                    // Check if expired/revoked
                    if let revocationDate = transaction.revocationDate {
                        statusResult["purchaseState"] = PurchaseStateValue.canceled.rawValue
                        statusResult["isOwned"] = false
                        statusResult["expirationTime"] = Int(revocationDate.timeIntervalSince1970 * 1000)
                    } else if let expirationDate = transaction.expirationDate {
                        if expirationDate < Date() {
                            statusResult["purchaseState"] = PurchaseStateValue.canceled.rawValue
                            statusResult["isOwned"] = false
                        } else {
                            statusResult["purchaseState"] = PurchaseStateValue.purchased.rawValue
                        }
                        statusResult["expirationTime"] = Int(expirationDate.timeIntervalSince1970 * 1000)
                    } else {
                        statusResult["purchaseState"] = PurchaseStateValue.purchased.rawValue
                    }
                    
                    // Check subscription renewal status if it's a subscription
                    if let product = try? await Product.products(for: [args.productId]).first {
                        if product.type == .autoRenewable {
                            // Check subscription status
                            if let statuses = try? await product.subscription?.status {
                                for status in statuses {
                                    if status.state == .subscribed {
                                        statusResult["isAutoRenewing"] = true
                                    } else if status.state == .expired {
                                        statusResult["isAutoRenewing"] = false
                                        statusResult["purchaseState"] = PurchaseStateValue.canceled.rawValue
                                        statusResult["isOwned"] = false
                                    } else if status.state == .inGracePeriod {
                                        statusResult["isAutoRenewing"] = true
                                        statusResult["purchaseState"] = PurchaseStateValue.purchased.rawValue
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
        
        invoke.resolve(statusResult)
    }
    
    private func handleTransactionUpdate(_ result: VerificationResult<Transaction>) async {
        switch result {
        case .verified(let transaction):
            // Get product details
            if let product = try? await Product.products(for: [transaction.productID]).first {
                let purchase = await createPurchaseObject(from: transaction, product: product)
                
                // Emit event - convert to JSObject-compatible format
                trigger("purchaseUpdated", data: purchase as! JSObject)
            }
            
            // Always finish transactions
            await transaction.finish()
            
        case .unverified(_, _):
            // Handle unverified transaction
            break
        }
    }
    
    private func createPurchaseObject(from transaction: Transaction, product: Product) async -> JsonObject {
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
            "isAcknowledged": true,  // Always true on iOS
            "originalJson": "",      // Not available in StoreKit 2
            "signature": ""          // Not available in StoreKit 2
        ]
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
        if #available(iOS 16.0, *) {
            return product.priceFormatStyle.locale.currency?.identifier ?? ""
        } else {
            // Fallback for iOS 15: currency code not directly available
            return ""
        }
    }
}

@_cdecl("init_plugin_iap")
func initPlugin() -> Plugin {
    if #available(iOS 15.0, *) {
        return IapPlugin()
    } else {
        // Return a dummy plugin for older iOS versions
        class DummyPlugin: Plugin {
            @objc func initialize(_ invoke: Invoke) {
                invoke.reject("IAP requires iOS 15.0 or later")
            }
            @objc func getProducts(_ invoke: Invoke) {
                invoke.reject("IAP requires iOS 15.0 or later")
            }
            @objc func purchase(_ invoke: Invoke) {
                invoke.reject("IAP requires iOS 15.0 or later")
            }
            @objc func restorePurchases(_ invoke: Invoke) {
                invoke.reject("IAP requires iOS 15.0 or later")
            }
            @objc func getPurchaseHistory(_ invoke: Invoke) {
                invoke.reject("IAP requires iOS 15.0 or later")
            }
            @objc func acknowledgePurchase(_ invoke: Invoke) {
                invoke.reject("IAP requires iOS 15.0 or later")
            }
            @objc func getProductStatus(_ invoke: Invoke) {
                invoke.reject("IAP requires iOS 15.0 or later")
            }
        }
        return DummyPlugin()
    }
}
