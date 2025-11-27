import XCTest
import StoreKit
import StoreKitTest
import Tauri
@testable import tauri_plugin_iap

// MARK: - Test Invoke Helper

/// Helper to create Invoke instances for testing and capture responses
class TestInvokeResult {
    var resolvedPayload: String?
    var rejectedPayload: String?
    var didResolve = false
    var didReject = false

    /// Parse resolved payload as JSON dictionary
    func getResolvedJson() -> [String: Any]? {
        guard let payload = resolvedPayload,
              let data = payload.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return json
    }

    /// Parse rejected payload to get error message
    func getRejectedMessage() -> String? {
        guard let payload = rejectedPayload,
              let data = payload.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return json["message"] as? String
    }
}

/// Create an Invoke instance for testing
func createTestInvoke(command: String, args: [String: Any]) -> (Invoke, TestInvokeResult) {
    let result = TestInvokeResult()
    let callbackId: UInt64 = 0
    let errorId: UInt64 = 1

    let argsJson = try! JSONSerialization.data(withJSONObject: args)
    let argsString = String(data: argsJson, encoding: .utf8)!

    let invoke = Invoke(
        command: command,
        callback: callbackId,
        error: errorId,
        sendResponse: { (fn: UInt64, payload: String?) in
            if fn == callbackId {
                result.didResolve = true
                result.resolvedPayload = payload
            } else {
                result.didReject = true
                result.rejectedPayload = payload
            }
        },
        sendChannelData: { (_, _) in },
        data: argsString
    )

    return (invoke, result)
}

// MARK: - Argument Classes Tests

final class GetProductsArgsTests: XCTestCase {
    func testDecoding() throws {
        let json = """
        {
            "productIds": ["product1", "product2", "product3"],
            "productType": "inapp"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(GetProductsArgs.self, from: data)

        XCTAssertEqual(args.productIds, ["product1", "product2", "product3"])
        XCTAssertEqual(args.productType, "inapp")
    }

    func testDecodingWithSubscriptionType() throws {
        let json = """
        {
            "productIds": ["sub_monthly"],
            "productType": "subs"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(GetProductsArgs.self, from: data)

        XCTAssertEqual(args.productIds, ["sub_monthly"])
        XCTAssertEqual(args.productType, "subs")
    }

    func testDecodingEmptyProductIds() throws {
        let json = """
        {
            "productIds": [],
            "productType": "inapp"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(GetProductsArgs.self, from: data)

        XCTAssertTrue(args.productIds.isEmpty)
        XCTAssertEqual(args.productType, "inapp")
    }

    func testDecodingFailsWithMissingProductIds() {
        let json = """
        {
            "productType": "inapp"
        }
        """
        let data = json.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(GetProductsArgs.self, from: data))
    }

    func testDecodingFailsWithMissingProductType() {
        let json = """
        {
            "productIds": ["product1"]
        }
        """
        let data = json.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(GetProductsArgs.self, from: data))
    }
}

final class PurchaseArgsTests: XCTestCase {
    func testDecodingMinimal() throws {
        let json = """
        {
            "productId": "com.example.product"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(PurchaseArgs.self, from: data)

        XCTAssertEqual(args.productId, "com.example.product")
        XCTAssertNil(args.productType)
        XCTAssertNil(args.offerToken)
        XCTAssertNil(args.appAccountToken)
    }

    func testDecodingFull() throws {
        let json = """
        {
            "productId": "com.example.subscription",
            "productType": "subs",
            "offerToken": "intro_offer_123",
            "appAccountToken": "550e8400-e29b-41d4-a716-446655440000"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(PurchaseArgs.self, from: data)

        XCTAssertEqual(args.productId, "com.example.subscription")
        XCTAssertEqual(args.productType, "subs")
        XCTAssertEqual(args.offerToken, "intro_offer_123")
        XCTAssertEqual(args.appAccountToken, "550e8400-e29b-41d4-a716-446655440000")
    }

    func testDecodingFailsWithMissingProductId() {
        let json = """
        {
            "productType": "inapp"
        }
        """
        let data = json.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(PurchaseArgs.self, from: data))
    }
}

final class RestorePurchasesArgsTests: XCTestCase {
    func testDecodingWithProductType() throws {
        let json = """
        {
            "productType": "subs"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(RestorePurchasesArgs.self, from: data)

        XCTAssertEqual(args.productType, "subs")
    }

    func testDecodingEmptyObject() throws {
        let json = "{}"
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(RestorePurchasesArgs.self, from: data)

        XCTAssertNil(args.productType)
    }
}

final class AcknowledgePurchaseArgsTests: XCTestCase {
    func testDecoding() throws {
        let json = """
        {
            "purchaseToken": "token_abc123xyz"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(AcknowledgePurchaseArgs.self, from: data)

        XCTAssertEqual(args.purchaseToken, "token_abc123xyz")
    }

    func testDecodingFailsWithMissingToken() {
        let json = "{}"
        let data = json.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(AcknowledgePurchaseArgs.self, from: data))
    }
}

final class GetProductStatusArgsTests: XCTestCase {
    func testDecodingMinimal() throws {
        let json = """
        {
            "productId": "com.example.premium"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(GetProductStatusArgs.self, from: data)

        XCTAssertEqual(args.productId, "com.example.premium")
        XCTAssertNil(args.productType)
    }

    func testDecodingWithProductType() throws {
        let json = """
        {
            "productId": "com.example.subscription",
            "productType": "subs"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(GetProductStatusArgs.self, from: data)

        XCTAssertEqual(args.productId, "com.example.subscription")
        XCTAssertEqual(args.productType, "subs")
    }

    func testDecodingFailsWithMissingProductId() {
        let json = """
        {
            "productType": "inapp"
        }
        """
        let data = json.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(GetProductStatusArgs.self, from: data))
    }
}

// MARK: - PurchaseStateValue Tests

final class PurchaseStateValueTests: XCTestCase {
    func testPurchasedValue() {
        XCTAssertEqual(PurchaseStateValue.purchased.rawValue, 0)
    }

    func testCanceledValue() {
        XCTAssertEqual(PurchaseStateValue.canceled.rawValue, 1)
    }

    func testPendingValue() {
        XCTAssertEqual(PurchaseStateValue.pending.rawValue, 2)
    }

    func testInitFromRawValue() {
        XCTAssertEqual(PurchaseStateValue(rawValue: 0), .purchased)
        XCTAssertEqual(PurchaseStateValue(rawValue: 1), .canceled)
        XCTAssertEqual(PurchaseStateValue(rawValue: 2), .pending)
        XCTAssertNil(PurchaseStateValue(rawValue: 3))
        XCTAssertNil(PurchaseStateValue(rawValue: -1))
    }
}

// MARK: - IapPlugin Function Tests

@available(iOS 15.0, *)
final class IapPluginFunctionTests: XCTestCase {
    var session: SKTestSession?
    var plugin: IapPlugin!

    override func setUp() async throws {
        try await super.setUp()

        do {
            let url = try XCTUnwrap(
                Bundle.module.url(forResource: "TestProducts", withExtension: "storekit")
            )
            
            session = try SKTestSession(contentsOf: url)
            
            session?.resetToDefaultState()
            session?.disableDialogs = true
            session?.clearTransactions()
        } catch {
            XCTFail("Failed to load StoreKit configuration: \(error). Make sure TestProducts.storekit is in the test bundle.")
        }

        plugin = IapPlugin()
    }

    override func tearDown() async throws {
        session?.clearTransactions()
        session = nil
        plugin = nil
        try await super.tearDown()
    }

    // MARK: - initialize() Tests

    func testInitializeReturnsSuccess() throws {
        let (invoke, result) = createTestInvoke(command: "initialize", args: [:])

        try plugin.initialize(invoke)

        XCTAssertTrue(result.didResolve)
        XCTAssertFalse(result.didReject)

        let json = result.getResolvedJson()
        XCTAssertEqual(json?["success"] as? Bool, true)
    }

    // MARK: - getProducts() Tests

    func testGetProductsReturnsProducts() async throws {
        let (invoke, result) = createTestInvoke(command: "getProducts", args: [
            "productIds": ["com.test.removeads", "com.test.premium"],
            "productType": "inapp"
        ])

        try await plugin.getProducts(invoke)

        XCTAssertTrue(result.didResolve)
        XCTAssertFalse(result.didReject)

        let json = result.getResolvedJson()
        let products = json?["products"] as? [[String: Any]]
        XCTAssertNotNil(products)
        XCTAssertEqual(products?.count, 2)

        if let firstProduct = products?.first {
            XCTAssertNotNil(firstProduct["productId"])
            XCTAssertNotNil(firstProduct["title"])
            XCTAssertNotNil(firstProduct["description"])
            XCTAssertNotNil(firstProduct["formattedPrice"])
        }
    }

    func testGetProductsWithSubscription() async throws {
        let (invoke, result) = createTestInvoke(command: "getProducts", args: [
            "productIds": ["com.test.premium.monthly"],
            "productType": "subs"
        ])

        try await plugin.getProducts(invoke)

        XCTAssertTrue(result.didResolve)

        let json = result.getResolvedJson()
        let products = json?["products"] as? [[String: Any]]
        XCTAssertEqual(products?.count, 1)

        if let subscription = products?.first {
            XCTAssertEqual(subscription["productId"] as? String, "com.test.premium.monthly")
            XCTAssertNotNil(subscription["subscriptionOfferDetails"])
        }
    }

    func testGetProductsWithNonExistentProduct() async throws {
        let (invoke, result) = createTestInvoke(command: "getProducts", args: [
            "productIds": ["com.test.nonexistent"],
            "productType": "inapp"
        ])

        try await plugin.getProducts(invoke)

        XCTAssertTrue(result.didResolve)

        let json = result.getResolvedJson()
        let products = json?["products"] as? [[String: Any]]
        XCTAssertEqual(products?.count, 0)
    }

    func testGetProductsWithEmptyArray() async throws {
        let (invoke, result) = createTestInvoke(command: "getProducts", args: [
            "productIds": [],
            "productType": "inapp"
        ])

        try await plugin.getProducts(invoke)

        XCTAssertTrue(result.didResolve)

        let json = result.getResolvedJson()
        let products = json?["products"] as? [[String: Any]]
        XCTAssertEqual(products?.count, 0)
    }

    func testGetProductsWithConsumable() async throws {
        let (invoke, result) = createTestInvoke(command: "getProducts", args: [
            "productIds": ["com.test.coins100"],
            "productType": "inapp"
        ])

        try await plugin.getProducts(invoke)

        XCTAssertTrue(result.didResolve)

        let json = result.getResolvedJson()
        let products = json?["products"] as? [[String: Any]]
        XCTAssertEqual(products?.count, 1)

        if let product = products?.first {
            XCTAssertEqual(product["productId"] as? String, "com.test.coins100")
            XCTAssertEqual(product["title"] as? String, "100 Coins")
        }
    }

    // MARK: - purchase() Tests

    func testPurchaseWithInvalidAppAccountToken() async throws {
        let (invoke, result) = createTestInvoke(command: "purchase", args: [
            "productId": "com.test.premium",
            "appAccountToken": "invalid-uuid"
        ])

        try await plugin.purchase(invoke)

        XCTAssertTrue(result.didReject)
        let message = result.getRejectedMessage()
        XCTAssertTrue(message?.contains("Invalid appAccountToken") ?? false)
    }

    func testPurchaseNonExistentProduct() async throws {
        let (invoke, result) = createTestInvoke(command: "purchase", args: [
            "productId": "com.test.nonexistent"
        ])

        try await plugin.purchase(invoke)

        XCTAssertTrue(result.didReject)
        XCTAssertEqual(result.getRejectedMessage(), "Product not found")
    }

    // MARK: - acknowledgePurchase() Tests

    func testAcknowledgePurchaseAlwaysSucceeds() throws {
        let (invoke, result) = createTestInvoke(command: "acknowledgePurchase", args: [
            "purchaseToken": "any_token_12345"
        ])

        try plugin.acknowledgePurchase(invoke)

        XCTAssertTrue(result.didResolve)
        let json = result.getResolvedJson()
        XCTAssertEqual(json?["success"] as? Bool, true)
    }

    // MARK: - restorePurchases() Tests

    func testRestorePurchasesEmpty() async throws {
        let (invoke, result) = createTestInvoke(command: "restorePurchases", args: [:])

        try await plugin.restorePurchases(invoke)

        XCTAssertTrue(result.didResolve)

        let json = result.getResolvedJson()
        let purchases = json?["purchases"] as? [[String: Any]]
        XCTAssertNotNil(purchases)
        XCTAssertEqual(purchases?.count, 0)
    }

    // MARK: - getPurchaseHistory() Tests

    func testGetPurchaseHistoryEmpty() async throws {
        let (invoke, result) = createTestInvoke(command: "getPurchaseHistory", args: [:])

        try await plugin.getPurchaseHistory(invoke)

        XCTAssertTrue(result.didResolve)

        let json = result.getResolvedJson()
        let history = json?["history"] as? [[String: Any]]
        XCTAssertNotNil(history)
    }

    // MARK: - getProductStatus() Tests

    func testGetProductStatusNotOwned() async throws {
        let (invoke, result) = createTestInvoke(command: "getProductStatus", args: [
            "productId": "com.test.premium"
        ])

        try await plugin.getProductStatus(invoke)

        XCTAssertTrue(result.didResolve)
        let json = result.getResolvedJson()
        XCTAssertEqual(json?["productId"] as? String, "com.test.premium")
        XCTAssertEqual(json?["isOwned"] as? Bool, false)
    }
}

// MARK: - Edge Cases Tests

final class ArgsEdgeCasesTests: XCTestCase {
    func testGetProductsArgsWithSpecialCharacters() throws {
        let json = """
        {
            "productIds": ["com.example.product-v2", "com.example.product_v3", "com.example.product.v4"],
            "productType": "inapp"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(GetProductsArgs.self, from: data)

        XCTAssertEqual(args.productIds.count, 3)
        XCTAssertTrue(args.productIds.contains("com.example.product-v2"))
        XCTAssertTrue(args.productIds.contains("com.example.product_v3"))
        XCTAssertTrue(args.productIds.contains("com.example.product.v4"))
    }

    func testPurchaseArgsWithValidUUID() throws {
        let validUUID = "550e8400-e29b-41d4-a716-446655440000"
        let json = """
        {
            "productId": "com.example.product",
            "appAccountToken": "\(validUUID)"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(PurchaseArgs.self, from: data)

        XCTAssertEqual(args.appAccountToken, validUUID)
        XCTAssertNotNil(UUID(uuidString: args.appAccountToken!))
    }

    func testPurchaseArgsWithInvalidUUID() throws {
        let invalidUUID = "not-a-valid-uuid"
        let json = """
        {
            "productId": "com.example.product",
            "appAccountToken": "\(invalidUUID)"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(PurchaseArgs.self, from: data)

        XCTAssertEqual(args.appAccountToken, invalidUUID)
        XCTAssertNil(UUID(uuidString: args.appAccountToken!))
    }

    func testGetProductsArgsWithLargeProductList() throws {
        let productIds = (1...100).map { "product_\($0)" }
        let productIdsJson = productIds.map { "\"\($0)\"" }.joined(separator: ", ")
        let json = """
        {
            "productIds": [\(productIdsJson)],
            "productType": "inapp"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(GetProductsArgs.self, from: data)

        XCTAssertEqual(args.productIds.count, 100)
        XCTAssertEqual(args.productIds.first, "product_1")
        XCTAssertEqual(args.productIds.last, "product_100")
    }

    func testAcknowledgePurchaseArgsWithLongToken() throws {
        let longToken = String(repeating: "a", count: 1000)
        let json = """
        {
            "purchaseToken": "\(longToken)"
        }
        """
        let data = json.data(using: .utf8)!
        let args = try JSONDecoder().decode(AcknowledgePurchaseArgs.self, from: data)

        XCTAssertEqual(args.purchaseToken.count, 1000)
    }
}
