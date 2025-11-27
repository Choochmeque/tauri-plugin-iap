package app.tauri.iap

import org.junit.Test
import org.junit.Assert.*

/**
 * Unit tests for IAP Plugin data classes and business logic.
 */
class IapPluginTest {

    @Test
    fun testGetProductsArgs_defaultValues() {
        val args = GetProductsArgs()
        assertEquals(emptyList<String>(), args.productIds)
        assertEquals("subs", args.productType)
    }

    @Test
    fun testGetProductsArgs_withCustomValues() {
        val args = GetProductsArgs().apply {
            productIds = listOf("product1", "product2")
            productType = "inapp"
        }
        assertEquals(2, args.productIds.size)
        assertEquals("product1", args.productIds[0])
        assertEquals("product2", args.productIds[1])
        assertEquals("inapp", args.productType)
    }

    @Test
    fun testPurchaseArgs_defaultValues() {
        val args = PurchaseArgs()
        assertEquals("", args.productId)
        assertEquals("subs", args.productType)
        assertNull(args.offerToken)
        assertNull(args.obfuscatedAccountId)
        assertNull(args.obfuscatedProfileId)
    }

    @Test
    fun testPurchaseArgs_withCustomValues() {
        val args = PurchaseArgs().apply {
            productId = "test_product"
            productType = "inapp"
            offerToken = "test_token"
            obfuscatedAccountId = "user123"
            obfuscatedProfileId = "profile456"
        }
        assertEquals("test_product", args.productId)
        assertEquals("inapp", args.productType)
        assertEquals("test_token", args.offerToken)
        assertEquals("user123", args.obfuscatedAccountId)
        assertEquals("profile456", args.obfuscatedProfileId)
    }

    @Test
    fun testRestorePurchasesArgs_defaultValues() {
        val args = RestorePurchasesArgs()
        assertEquals("subs", args.productType)
    }

    @Test
    fun testRestorePurchasesArgs_withCustomValue() {
        val args = RestorePurchasesArgs().apply {
            productType = "inapp"
        }
        assertEquals("inapp", args.productType)
    }

    @Test
    fun testAcknowledgePurchaseArgs_defaultValues() {
        val args = AcknowledgePurchaseArgs()
        assertNull(args.purchaseToken)
    }

    @Test
    fun testAcknowledgePurchaseArgs_withToken() {
        val args = AcknowledgePurchaseArgs().apply {
            purchaseToken = "test_token_123"
        }
        assertEquals("test_token_123", args.purchaseToken)
    }

    @Test
    fun testGetProductStatusArgs_defaultValues() {
        val args = GetProductStatusArgs()
        assertEquals("", args.productId)
        assertEquals("subs", args.productType)
    }

    @Test
    fun testGetProductStatusArgs_withCustomValues() {
        val args = GetProductStatusArgs().apply {
            productId = "premium_sub"
            productType = "inapp"
        }
        assertEquals("premium_sub", args.productId)
        assertEquals("inapp", args.productType)
    }

    @Test
    fun testPurchaseStateConstants() {
        assertEquals(0, IapPlugin.PURCHASE_STATE_PURCHASED)
        assertEquals(1, IapPlugin.PURCHASE_STATE_CANCELED)
        assertEquals(2, IapPlugin.PURCHASE_STATE_PENDING)
    }

    @Test
    fun testPurchaseStateConstants_areUnique() {
        val states = setOf(
            IapPlugin.PURCHASE_STATE_PURCHASED,
            IapPlugin.PURCHASE_STATE_CANCELED,
            IapPlugin.PURCHASE_STATE_PENDING
        )
        assertEquals(3, states.size)
    }
}
