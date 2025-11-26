package app.tauri.iap

import android.content.Context
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.Assert.*
import org.junit.Before

/**
 * Instrumented tests for IAP Plugin.
 * These tests run on an Android device or emulator.
 */
@RunWith(AndroidJUnit4::class)
class IapPluginInstrumentedTest {

    private lateinit var context: Context

    @Before
    fun setup() {
        context = InstrumentationRegistry.getInstrumentation().targetContext
    }

    @Test
    fun testContextPackageName() {
        assertEquals("app.tauri.iap.test", context.packageName)
    }

    @Test
    fun testGetProductsArgs_serialization() {
        val args = GetProductsArgs().apply {
            productIds = listOf("product1", "product2", "product3")
            productType = "inapp"
        }

        assertEquals(3, args.productIds.size)
        assertEquals("inapp", args.productType)
        assertTrue(args.productIds.contains("product1"))
        assertTrue(args.productIds.contains("product2"))
        assertTrue(args.productIds.contains("product3"))
    }

    @Test
    fun testPurchaseArgs_allFields() {
        val args = PurchaseArgs().apply {
            productId = "test.product.id"
            productType = "subs"
            offerToken = "offer_token_xyz"
            obfuscatedAccountId = "account_123"
            obfuscatedProfileId = "profile_456"
        }

        assertEquals("test.product.id", args.productId)
        assertEquals("subs", args.productType)
        assertEquals("offer_token_xyz", args.offerToken)
        assertEquals("account_123", args.obfuscatedAccountId)
        assertEquals("profile_456", args.obfuscatedProfileId)
    }

    @Test
    fun testRestorePurchasesArgs_bothProductTypes() {
        val subsArgs = RestorePurchasesArgs().apply {
            productType = "subs"
        }
        val inappArgs = RestorePurchasesArgs().apply {
            productType = "inapp"
        }

        assertEquals("subs", subsArgs.productType)
        assertEquals("inapp", inappArgs.productType)
    }

    @Test
    fun testAcknowledgePurchaseArgs_withNullToken() {
        val args = AcknowledgePurchaseArgs()
        assertNull(args.purchaseToken)
    }

    @Test
    fun testAcknowledgePurchaseArgs_withValidToken() {
        val token = "valid_purchase_token_12345"
        val args = AcknowledgePurchaseArgs().apply {
            purchaseToken = token
        }
        assertEquals(token, args.purchaseToken)
    }

    @Test
    fun testGetProductStatusArgs_configuration() {
        val args = GetProductStatusArgs().apply {
            productId = "premium.subscription"
            productType = "subs"
        }

        assertEquals("premium.subscription", args.productId)
        assertEquals("subs", args.productType)
    }

    @Test
    fun testPurchaseStateConstants_values() {
        // Verify the purchase state constants match expected values
        assertEquals(0, IapPlugin.PURCHASE_STATE_PURCHASED)
        assertEquals(1, IapPlugin.PURCHASE_STATE_CANCELED)
        assertEquals(2, IapPlugin.PURCHASE_STATE_PENDING)
    }

    @Test
    fun testPurchaseStateConstants_areDistinct() {
        val allStates = listOf(
            IapPlugin.PURCHASE_STATE_PURCHASED,
            IapPlugin.PURCHASE_STATE_CANCELED,
            IapPlugin.PURCHASE_STATE_PENDING
        )

        // Verify all states are distinct
        assertEquals(allStates.size, allStates.toSet().size)
    }

    @Test
    fun testGetProductsArgs_withEmptyList() {
        val args = GetProductsArgs().apply {
            productIds = emptyList()
        }

        assertTrue(args.productIds.isEmpty())
        assertEquals("subs", args.productType) // default value
    }

    @Test
    fun testGetProductsArgs_withLargeList() {
        val largeList = (1..100).map { "product_$it" }
        val args = GetProductsArgs().apply {
            productIds = largeList
            productType = "inapp"
        }

        assertEquals(100, args.productIds.size)
        assertEquals("product_1", args.productIds.first())
        assertEquals("product_100", args.productIds.last())
    }
}
