import XCTest
import StoreKit
import StoreKitTest
@testable import tauri_plugin_iap

// MARK: - Test Helpers

/// Parses a JSON string into a dictionary.
private func parseJSON(_ jsonString: String) -> JsonObject? {
    guard let data = jsonString.data(using: .utf8),
          let json = try? JSONSerialization.jsonObject(with: data) as? JsonObject else {
        return nil
    }
    return json
}

final class PluginTests: XCTestCase {
    var plugin: IapPlugin!

    override func setUp() {
        super.setUp()
        plugin = initPlugin()
    }

    override func tearDown() {
        plugin = nil
        super.tearDown()
    }

    // MARK: - PurchaseStateValue Tests

    func testPurchaseStateValueRawValues() {
        XCTAssertEqual(PurchaseStateValue.purchased.rawValue, 0)
        XCTAssertEqual(PurchaseStateValue.canceled.rawValue, 1)
        XCTAssertEqual(PurchaseStateValue.pending.rawValue, 2)
    }

    func testPurchaseStateValueFromRawValue() {
        XCTAssertEqual(PurchaseStateValue(rawValue: 0), .purchased)
        XCTAssertEqual(PurchaseStateValue(rawValue: 1), .canceled)
        XCTAssertEqual(PurchaseStateValue(rawValue: 2), .pending)
        XCTAssertNil(PurchaseStateValue(rawValue: 99))
    }

    // MARK: - appAccountToken Tests
    //
    // These run without a StoreKit daemon because `purchase()` validates the
    // token before it fetches the product — which is also why a malformed
    // token costs no round trip.

    func testPurchaseRejectsNonUUIDAppAccountToken() async {
        do {
            _ = try await plugin.purchase(
                productId: RustString("com.test.premium"),
                productType: RustString("inapp"),
                offerToken: nil,
                appAccountToken: RustString("not-a-uuid")
            )
            XCTFail("Expected a non-UUID appAccountToken to be rejected")
        } catch let error as FFIResult {
            guard case .Err(let message) = error else {
                return XCTFail("Expected FFIResult.Err")
            }
            XCTAssertTrue(
                message.toString().contains("Invalid appAccountToken"),
                "Expected an appAccountToken error, got: \(message.toString())")
        } catch {
            XCTFail("Expected FFIResult, got \(error)")
        }
    }

    func testPurchaseAcceptsValidUUIDAppAccountToken() async {
        // A well-formed UUID must get PAST validation. Without a StoreKit
        // daemon the call still fails at the product fetch, so the assertion is
        // that it fails for that reason and not for the token.
        do {
            _ = try await plugin.purchase(
                productId: RustString("com.test.premium"),
                productType: RustString("inapp"),
                offerToken: nil,
                appAccountToken: RustString(UUID().uuidString)
            )
        } catch let error as FFIResult {
            guard case .Err(let message) = error else { return }
            XCTAssertFalse(
                message.toString().contains("Invalid appAccountToken"),
                "A valid UUID was rejected as an appAccountToken")
        } catch {
            // Any non-FFIResult failure is unrelated to token validation.
        }
    }

    /// Lowercase is what Supabase and most UUID libraries emit, uppercase is
    /// what Apple echoes back in the signed transaction. Both must parse, or
    /// the binding check breaks for one half of the world.
    func testPurchaseAcceptsUUIDInEitherCase() async {
        for token in ["3f7c1f6e-1b2a-4c3d-9e8f-0a1b2c3d4e5f",
                      "3F7C1F6E-1B2A-4C3D-9E8F-0A1B2C3D4E5F"] {
            do {
                _ = try await plugin.purchase(
                    productId: RustString("com.test.premium"),
                    productType: RustString("inapp"),
                    offerToken: nil,
                    appAccountToken: RustString(token)
                )
            } catch let error as FFIResult {
                guard case .Err(let message) = error else { continue }
                XCTAssertFalse(
                    message.toString().contains("Invalid appAccountToken"),
                    "UUID \(token) was rejected")
            } catch {
                continue
            }
        }
    }

    // MARK: - Pending Purchase Tests

    /// `.pending` (Ask to Buy, SCA step-up) must come back as a purchase in the
    /// pending state rather than an error, and the shape has to deserialize
    /// into the `Purchase` struct in src/models.rs.
    func testPendingPurchaseObjectShape() {
        let pending = plugin.pendingPurchaseObject(productId: "com.test.premium")

        XCTAssertEqual(pending["purchaseState"] as? Int, PurchaseStateValue.pending.rawValue)
        XCTAssertEqual(pending["productId"] as? String, "com.test.premium")
        XCTAssertEqual(pending["isAcknowledged"] as? Bool, false)
        XCTAssertEqual(pending["isAutoRenewing"] as? Bool, false)
        XCTAssertEqual(pending["purchaseToken"] as? String, "")
        XCTAssertEqual(pending["purchaseTime"] as? Int, 0)

        // Every non-optional field of `Purchase` must be present or the Rust
        // side fails to deserialize and the pending case turns back into an
        // opaque error — the exact thing this is meant to fix.
        for key in ["orderId", "packageName", "productId", "purchaseTime", "purchaseToken",
                    "purchaseState", "isAutoRenewing", "isAcknowledged", "originalJson",
                    "signature", "originalId"] {
            XCTAssertNotNil(pending[key], "Missing required Purchase field: \(key)")
        }

        XCTAssertTrue(JSONSerialization.isValidJSONObject(pending))
    }

    // MARK: - Finish Behavior Tests

    /// Acknowledging a token that is not outstanding is the normal case when
    /// the plugin finishes automatically, and it is also what a retry looks
    /// like. It must succeed rather than error.
    func testFinishTransactionWithUnknownTokenSucceeds() async throws {
        let json = try await plugin.finishTransaction(purchaseToken: RustString("0"))
        XCTAssertEqual(parseJSON(json)?["finished"] as? Bool, true)
    }

    func testDefaultsToFinishingAutomatically() {
        // The default has to stay `true`: every release before this option
        // existed finished transactions itself, and an app that upgrades
        // without opting in must not silently stop acknowledging.
        XCTAssertTrue(IapPlugin().finishesTransactionsAutomaticallyForTesting)
        XCTAssertTrue(initPlugin().finishesTransactionsAutomaticallyForTesting)
        XCTAssertFalse(
            initPlugin(finishTransactionsAutomatically: false)
                .finishesTransactionsAutomaticallyForTesting)
    }

}

// MARK: - StoreKit Integration Tests

@available(macOS 12.0, *)
final class StoreKitTests: XCTestCase {
    var session: SKTestSession!
    var plugin: IapPlugin!

    override func setUp() async throws {
        try await super.setUp()

        plugin = initPlugin()

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
        plugin = nil
        try await super.tearDown()
    }

    // MARK: - getProducts Tests

    func testGetProductsReturnsProducts() async throws {
        // TODO: fix it somehow
        throw XCTSkip("Skipping due to StoreKit daemon unavailability")

        let productIds = RustVec<RustString>()
        productIds.push(value: RustString("com.test.removeads"))
        productIds.push(value: RustString("com.test.premium"))

        let jsonString = try await plugin.getProducts(productIds: productIds, productType: RustString("inapp"))
        let json = try XCTUnwrap(parseJSON(jsonString))
        let products = try XCTUnwrap(json["products"] as? [JsonObject])

        XCTAssertEqual(products.count, 2)

        if let firstProduct = products.first {
            XCTAssertNotNil(firstProduct["productId"])
            XCTAssertNotNil(firstProduct["title"])
            XCTAssertNotNil(firstProduct["description"])
            XCTAssertNotNil(firstProduct["formattedPrice"])
        }
    }

    func testGetProductsWithSubscription() async throws {
        // TODO: fix it somehow
        throw XCTSkip("Skipping due to StoreKit daemon unavailability")

        let productIds = RustVec<RustString>()
        productIds.push(value: RustString("com.test.premium.monthly"))

        let jsonString = try await plugin.getProducts(productIds: productIds, productType: RustString("subs"))
        let json = try XCTUnwrap(parseJSON(jsonString))
        let products = try XCTUnwrap(json["products"] as? [JsonObject])

        XCTAssertEqual(products.count, 1)

        if let subscription = products.first {
            XCTAssertEqual(subscription["productId"] as? String, "com.test.premium.monthly")
            XCTAssertNotNil(subscription["subscriptionOfferDetails"])
        }
    }

    func testGetProductsWithNonExistentProduct() async throws {
        let productIds = RustVec<RustString>()
        productIds.push(value: RustString("com.test.nonexistent"))

        let jsonString = try await plugin.getProducts(productIds: productIds, productType: RustString("inapp"))
        let json = try XCTUnwrap(parseJSON(jsonString))
        let products = try XCTUnwrap(json["products"] as? [JsonObject])

        XCTAssertEqual(products.count, 0)
    }

    func testGetProductsWithEmptyArray() async throws {
        let productIds = RustVec<RustString>()

        let jsonString = try await plugin.getProducts(productIds: productIds, productType: RustString("inapp"))
        let json = try XCTUnwrap(parseJSON(jsonString))
        let products = try XCTUnwrap(json["products"] as? [JsonObject])

        XCTAssertEqual(products.count, 0)
    }

    func testGetProductsWithConsumable() async throws {
        // TODO: fix it somehow
        throw XCTSkip("Skipping due to StoreKit daemon unavailability")

        let productIds = RustVec<RustString>()
        productIds.push(value: RustString("com.test.coins100"))

        let jsonString = try await plugin.getProducts(productIds: productIds, productType: RustString("inapp"))
        let json = try XCTUnwrap(parseJSON(jsonString))
        let products = try XCTUnwrap(json["products"] as? [JsonObject])

        XCTAssertEqual(products.count, 1)

        if let product = products.first {
            XCTAssertEqual(product["productId"] as? String, "com.test.coins100")
            XCTAssertEqual(product["title"] as? String, "100 Coins")
        }
    }
}
