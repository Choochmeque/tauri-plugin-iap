import XCTest
import StoreKit
import StoreKitTest
@testable import tauri_plugin_iap

// MARK: - Test Helpers

private func isResultOk(_ result: FFIResult) -> Bool {
    if case .Ok = result { return true }
    return false
}

private func getResultString(_ result: FFIResult) -> String? {
    switch result {
    case .Ok(let rustString):
        return rustString.toString()
    case .Err:
        return nil
    }
}

// Helper to avoid name conflict with XCTestCase.initialize
private func pluginInitialize() -> FFIResult { tauri_plugin_iap.initialize() }
private func pluginAcknowledgePurchase(purchaseToken: RustString) -> FFIResult { tauri_plugin_iap.acknowledgePurchase(purchaseToken: purchaseToken) }

final class PluginTests: XCTestCase {
    
    // MARK: - PurchaseStateValue Tests
    
    func testPurchaseStateValueRawValues() {
        XCTAssertEqual(PurchaseStateValue.purchased.rawValue, 0)
        XCTAssertEqual(PurchaseStateValue.canceled.rawValue, 1)
        XCTAssertEqual(PurchaseStateValue.pending.rawValue, 2)
    }
    
    // MARK: - Plugin Function Tests
    
    func testPluginInitialize() {
        let result = pluginInitialize()
        XCTAssertTrue(isResultOk(result))
        
        if let jsonString = getResultString(result),
           let data = jsonString.data(using: String.Encoding.utf8),
           let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            XCTAssertEqual(json["success"] as? Bool, true)
        } else {
            XCTFail("Failed to parse initialize response")
        }
    }
    
    func testAcknowledgePurchase() {
        // acknowledgePurchase is a no-op on macOS, should always succeed
        let result = pluginAcknowledgePurchase(purchaseToken: RustString("test_token"))
        XCTAssertTrue(isResultOk(result))
        
        if let jsonString = getResultString(result),
           let data = jsonString.data(using: String.Encoding.utf8),
           let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            XCTAssertEqual(json["success"] as? Bool, true)
        } else {
            XCTFail("Failed to parse acknowledgePurchase response")
        }
    }
    
    // MARK: - JSON Serialization Tests
    
    func testSerializeToJSONWithProducts() {
        // Test that our mock infrastructure allows testing JSON responses
        let result = pluginInitialize()
        guard case .Ok(let rustString) = result else {
            XCTFail("Expected Ok result")
            return
        }
        
        let jsonString = rustString.toString()
        XCTAssertFalse(jsonString.isEmpty)
        XCTAssertTrue(jsonString.contains("success"))
    }
    
    // MARK: - PurchaseStateValue Advanced Tests
    
    func testPurchaseStateValueFromRawValue() {
        XCTAssertEqual(PurchaseStateValue(rawValue: 0), .purchased)
        XCTAssertEqual(PurchaseStateValue(rawValue: 1), .canceled)
        XCTAssertEqual(PurchaseStateValue(rawValue: 2), .pending)
        XCTAssertNil(PurchaseStateValue(rawValue: 99))
    }
    
    // MARK: - Plugin Initialize Tests
    
    func testInitializeIsIdempotent() {
        // Multiple calls should all succeed
        for _ in 0..<5 {
            let result = pluginInitialize()
            XCTAssertTrue(isResultOk(result))
        }
    }
    
    func testInitializeReturnsValidJSON() {
        let result = pluginInitialize()
        guard let jsonString = getResultString(result),
              let data = jsonString.data(using: String.Encoding.utf8) else {
            XCTFail("Failed to get result string")
            return
        }
        
        XCTAssertNoThrow(try JSONSerialization.jsonObject(with: data))
    }
    
    // MARK: - Plugin AcknowledgePurchase Tests
    
    func testAcknowledgePurchaseWithEmptyToken() {
        let result = pluginAcknowledgePurchase(purchaseToken: RustString(""))
        XCTAssertTrue(isResultOk(result))
    }
    
    func testAcknowledgePurchaseWithLongToken() {
        let longToken = String(repeating: "token_", count: 100)
        let result = pluginAcknowledgePurchase(purchaseToken: RustString(longToken))
        XCTAssertTrue(isResultOk(result))
    }
    
    func testAcknowledgePurchaseReturnsValidJSON() {
        let result = pluginAcknowledgePurchase(purchaseToken: RustString("any_token"))
        guard let jsonString = getResultString(result),
              let data = jsonString.data(using: String.Encoding.utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            XCTFail("Failed to parse JSON")
            return
        }
        
        XCTAssertTrue(json.keys.contains("success"))
    }
}

// MARK: - StoreKit Integration Tests

@available(macOS 12.0, *)
final class StoreKitTests: XCTestCase {
    var session: SKTestSession!

    override func setUp() async throws {
        try await super.setUp()

        let url = try XCTUnwrap(
            Bundle.module.url(forResource: "TestProducts", withExtension: "storekit")
        )

        session = try SKTestSession(contentsOf: url)
        session.resetToDefaultState()
        session.disableDialogs = true
        session.clearTransactions()
    }

    override func tearDown() async throws {
        session.clearTransactions()
        session = nil
        try await super.tearDown()
    }

    // MARK: - getProducts Tests

    func testGetProductsReturnsProducts() async throws {
        // TODO: fix it somehow
        throw XCTSkip("Skipping testGetProductsWithEmptyArray due to StoreKit daemon unavailability")

        let productIds = RustVec<RustString>()
        productIds.push(value: RustString("com.test.removeads"))
        productIds.push(value: RustString("com.test.premium"))

        let result = await getProducts(productIds: productIds, productType: RustString("inapp"))

        XCTAssertTrue(isResultOk(result))

        guard let jsonString = getResultString(result),
              let data = jsonString.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let products = json["products"] as? [[String: Any]] else {
            XCTFail("Failed to parse products response")
            return
        }

        XCTAssertEqual(products.count, 2)

        // Check first product has expected fields
        if let firstProduct = products.first {
            XCTAssertNotNil(firstProduct["productId"])
            XCTAssertNotNil(firstProduct["title"])
            XCTAssertNotNil(firstProduct["description"])
            XCTAssertNotNil(firstProduct["formattedPrice"])
        }
    }

    func testGetProductsWithSubscription() async throws {
        // TODO: fix it somehow
        throw XCTSkip("Skipping testGetProductsWithEmptyArray due to StoreKit daemon unavailability")

        let productIds = RustVec<RustString>()
        productIds.push(value: RustString("com.test.premium.monthly"))

        let result = await getProducts(productIds: productIds, productType: RustString("subs"))

        XCTAssertTrue(isResultOk(result))

        guard let jsonString = getResultString(result),
              let data = jsonString.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let products = json["products"] as? [[String: Any]] else {
            XCTFail("Failed to parse products response")
            return
        }

        XCTAssertEqual(products.count, 1)

        if let subscription = products.first {
            XCTAssertEqual(subscription["productId"] as? String, "com.test.premium.monthly")
            XCTAssertNotNil(subscription["subscriptionOfferDetails"])
        }
    }

    func testGetProductsWithNonExistentProduct() async {
        let productIds = RustVec<RustString>()
        productIds.push(value: RustString("com.test.nonexistent"))

        let result = await getProducts(productIds: productIds, productType: RustString("inapp"))

        XCTAssertTrue(isResultOk(result))

        guard let jsonString = getResultString(result),
              let data = jsonString.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let products = json["products"] as? [[String: Any]] else {
            XCTFail("Failed to parse products response")
            return
        }

        XCTAssertEqual(products.count, 0)
    }

    func testGetProductsWithEmptyArray() async {
        let productIds = RustVec<RustString>()

        let result = await getProducts(productIds: productIds, productType: RustString("inapp"))

        XCTAssertTrue(isResultOk(result))

        guard let jsonString = getResultString(result),
              let data = jsonString.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let products = json["products"] as? [[String: Any]] else {
            XCTFail("Failed to parse products response")
            return
        }

        XCTAssertEqual(products.count, 0)
    }

    func testGetProductsWithConsumable() async throws {
        // TODO: fix it somehow
        throw XCTSkip("Skipping testGetProductsWithEmptyArray due to StoreKit daemon unavailability")

        let productIds = RustVec<RustString>()
        productIds.push(value: RustString("com.test.coins100"))

        let result = await getProducts(productIds: productIds, productType: RustString("inapp"))

        XCTAssertTrue(isResultOk(result))

        guard let jsonString = getResultString(result),
              let data = jsonString.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let products = json["products"] as? [[String: Any]] else {
            XCTFail("Failed to parse products response")
            return
        }

        XCTAssertEqual(products.count, 1)

        if let product = products.first {
            XCTAssertEqual(product["productId"] as? String, "com.test.coins100")
            XCTAssertEqual(product["title"] as? String, "100 Coins")
        }
    }
}
