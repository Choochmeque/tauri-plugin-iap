package app.tauri.iap

import android.app.Activity
import android.webkit.WebView
import app.tauri.Logger
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke
import com.android.billingclient.api.*
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray

@InvokeArg
class GetProductsArgs {
    var productIds: List<String> = emptyList()
    var productType: String = "subs" // "subs" or "inapp"
}

@InvokeArg
class PurchaseArgs {
    var productId: String = ""
    var productType: String = "subs" // "subs" or "inapp"
    var offerToken: String? = null
    var obfuscatedAccountId: String? = null
    var obfuscatedProfileId: String? = null
}

@InvokeArg
class RestorePurchasesArgs {
    var productType: String = "subs" // "subs" or "inapp"
}

@InvokeArg
class GetPurchaseHistoryArgs

@InvokeArg
class AcknowledgePurchaseArgs {
    var purchaseToken: String? = null
}

@InvokeArg
class GetProductStatusArgs {
    var productId: String = ""
    var productType: String = "subs" // "subs" or "inapp"
}

@TauriPlugin
class IapPlugin(private val activity: Activity): Plugin(activity), PurchasesUpdatedListener, BillingClientStateListener {
    private lateinit var billingClient: BillingClient
    private val coroutineScope = CoroutineScope(Dispatchers.Main)
    private var pendingPurchaseInvoke: Invoke? = null
    private val TAG = "IapPlugin"
    
    // Keep in sync with PurchaseState in guest-js/index.ts
    companion object {
        const val PURCHASE_STATE_PURCHASED = 0
        const val PURCHASE_STATE_CANCELED = 1
        const val PURCHASE_STATE_PENDING = 2

        fun translatePurchaseState(state: Int): Int = when(state) {
            Purchase.PurchaseState.PURCHASED -> PURCHASE_STATE_PURCHASED
            Purchase.PurchaseState.PENDING -> PURCHASE_STATE_PENDING
            else -> PURCHASE_STATE_CANCELED
        }

        fun translateProductType(productType: String): String = when(productType) {
            "inapp" -> BillingClient.ProductType.INAPP
            else -> BillingClient.ProductType.SUBS
        }
    }
    
    override fun load(webView: WebView) {
        super.load(webView)
        initializeBillingClient()
    }
    
    private fun initializeBillingClient() {
        var params = PendingPurchasesParams.newBuilder()
            .enableOneTimeProducts()
            .build();

        billingClient = BillingClient.newBuilder(activity)
            .setListener(this)
            .enablePendingPurchases(params)
            .enableAutoServiceReconnection()
            .build()

        billingClient.startConnection(object : BillingClientStateListener {
            override fun onBillingSetupFinished(billingResult: BillingResult) {
                if (billingResult.responseCode == BillingClient.BillingResponseCode.OK) {
                    Logger.info(TAG, "Billing setup finished successfully")
                } else {
                    Logger.error(TAG, "Billing setup failed: ${billingResult.debugMessage}")
                }
            }

            override fun onBillingServiceDisconnected() {
                Logger.debug(TAG, "Billing service disconnected")
            }
        })
    }
    
    @Command
    fun getProducts(invoke: Invoke) {
        val args = invoke.parseArgs(GetProductsArgs::class.java)
        
        if (!billingClient.isReady) {
            invoke.reject("Billing client not ready")
            return
        }
        
        val productType = translateProductType(args.productType)
        
        val productList = args.productIds.map { productId ->
            QueryProductDetailsParams.Product.newBuilder()
                .setProductId(productId)
                .setProductType(productType)
                .build()
        }
        
        val params = QueryProductDetailsParams.newBuilder()
            .setProductList(productList)
            .build()
        
        billingClient.queryProductDetailsAsync(params) { billingResult: BillingResult, productDetailsResult: QueryProductDetailsResult ->
            if (billingResult.responseCode == BillingClient.BillingResponseCode.OK) {
                val products = JSObject()
                val productsArray = productDetailsResult.productDetailsList?.map { productDetails ->
                    JSObject().apply {
                        put("productId", productDetails.productId)
                        put("title", productDetails.title)
                        put("description", productDetails.description)
                        put("productType", productDetails.productType)
                        
                        // For subscriptions, include offer details
                        if (productDetails.productType == BillingClient.ProductType.SUBS) {
                            val subscriptionOfferDetails = productDetails.subscriptionOfferDetails
                            if (!subscriptionOfferDetails.isNullOrEmpty()) {
                                val offers = subscriptionOfferDetails.map { offer ->
                                    JSObject().apply {
                                        put("offerToken", offer.offerToken)
                                        put("basePlanId", offer.basePlanId)
                                        put("offerId", offer.offerId)
                                        
                                        // Pricing phases
                                        val pricingPhases = offer.pricingPhases.pricingPhaseList.map { phase ->
                                            JSObject().apply {
                                                put("formattedPrice", phase.formattedPrice)
                                                put("priceCurrencyCode", phase.priceCurrencyCode)
                                                put("priceAmountMicros", phase.priceAmountMicros)
                                                put("billingPeriod", phase.billingPeriod)
                                                put("billingCycleCount", phase.billingCycleCount)
                                                put("recurrenceMode", phase.recurrenceMode)
                                            }
                                        }
                                        put("pricingPhases", JSONArray(pricingPhases))
                                    }
                                }
                                put("subscriptionOfferDetails", JSONArray(offers))
                            }
                        } else {
                            // For one-time products
                            val oneTimePurchaseOfferDetails = productDetails.oneTimePurchaseOfferDetails
                            if (oneTimePurchaseOfferDetails != null) {
                                put("formattedPrice", oneTimePurchaseOfferDetails.formattedPrice)
                                put("priceCurrencyCode", oneTimePurchaseOfferDetails.priceCurrencyCode)
                                put("priceAmountMicros", oneTimePurchaseOfferDetails.priceAmountMicros)
                            }
                        }
                    }
                }
                products.put("products", JSONArray(productsArray))
                invoke.resolve(products)
            } else {
                invoke.reject("Failed to fetch products: ${billingResult.debugMessage}")
            }
        }
    }
    
    @Command
    fun purchase(invoke: Invoke) {
        val args = invoke.parseArgs(PurchaseArgs::class.java)
        
        if (!billingClient.isReady) {
            invoke.reject("Billing client not ready")
            return
        }
        
        pendingPurchaseInvoke = invoke
        
        val productType = translateProductType(args.productType)
        
        // First, get the product details
        val productList = listOf(
            QueryProductDetailsParams.Product.newBuilder()
                .setProductId(args.productId)
                .setProductType(productType)
                .build()
        )
        
        val params = QueryProductDetailsParams.newBuilder()
            .setProductList(productList)
            .build()
        
        billingClient.queryProductDetailsAsync(params) { billingResult: BillingResult, productDetailsResult: QueryProductDetailsResult ->
            if (billingResult.responseCode == BillingClient.BillingResponseCode.OK && productDetailsResult.productDetailsList.isNotEmpty()) {
                val productDetails = productDetailsResult.productDetailsList[0]

                // Get offer token from args or from first available subscription offer
                val offerToken = args.offerToken ?: 
                    productDetails.subscriptionOfferDetails?.firstOrNull()?.offerToken
                
                val productDetailsParamsBuilder = BillingFlowParams.ProductDetailsParams.newBuilder()
                    .setProductDetails(productDetails)
                
                offerToken?.let { productDetailsParamsBuilder.setOfferToken(it) }
                
                val productDetailsParamsList = listOf(productDetailsParamsBuilder.build())
                
                val billingFlowParamsBuilder = BillingFlowParams.newBuilder()
                    .setProductDetailsParamsList(productDetailsParamsList)
                
                // Add obfuscated account ID if provided
                args.obfuscatedAccountId?.let { accountId ->
                    billingFlowParamsBuilder.setObfuscatedAccountId(accountId)
                }
                
                // Add obfuscated profile ID if provided
                args.obfuscatedProfileId?.let { profileId ->
                    billingFlowParamsBuilder.setObfuscatedProfileId(profileId)
                }
                
                val billingFlowParams = billingFlowParamsBuilder.build()
                
                val billingResult = billingClient.launchBillingFlow(activity, billingFlowParams)
                
                if (billingResult.responseCode != BillingClient.BillingResponseCode.OK) {
                    pendingPurchaseInvoke = null
                    invoke.reject("Failed to launch billing flow: ${billingResult.debugMessage}")
                }
            } else {
                pendingPurchaseInvoke = null
                invoke.reject("Product not found")
            }
        }
    }
    
    @Command
    fun restorePurchases(invoke: Invoke) {
        val args = invoke.parseArgs(RestorePurchasesArgs::class.java)
        
        if (!billingClient.isReady) {
            invoke.reject("Billing client not ready")
            return
        }
        
        val productType = translateProductType(args.productType)
        
        val params = QueryPurchasesParams.newBuilder()
            .setProductType(productType)
            .build()
        
        billingClient.queryPurchasesAsync(params) { billingResult, purchases ->
            if (billingResult.responseCode == BillingClient.BillingResponseCode.OK) {
                val purchasesArray = purchases.map { purchase ->
                    JSObject().apply {
                        put("orderId", purchase.orderId)
                        put("packageName", purchase.packageName)
                        put("productId", purchase.products.firstOrNull() ?: "")
                        put("purchaseTime", purchase.purchaseTime)
                        put("purchaseToken", purchase.purchaseToken)
                        put("purchaseState", translatePurchaseState(purchase.purchaseState))
                        put("isAutoRenewing", purchase.isAutoRenewing)
                        put("isAcknowledged", purchase.isAcknowledged)
                        put("originalJson", purchase.originalJson)
                        put("signature", purchase.signature)
                    }
                }

                val result = JSObject()
                result.put("purchases", JSONArray(purchasesArray))
                invoke.resolve(result)
            } else {
                invoke.reject("Failed to restore purchases: ${billingResult.debugMessage}")
            }
        }
    }
    
    @Command
    fun getPurchaseHistory(invoke: Invoke) {
        invoke.reject("Purchase history is not supported")
    }
    
    @Command
    fun acknowledgePurchase(invoke: Invoke) {
        val purchaseToken = invoke.parseArgs(AcknowledgePurchaseArgs::class.java).purchaseToken
        
        if (purchaseToken == null) {
            invoke.reject("Purchase token is required")
            return
        }
        
        if (!billingClient.isReady) {
            invoke.reject("Billing client not ready")
            return
        }
        
        val acknowledgePurchaseParams = AcknowledgePurchaseParams.newBuilder()
            .setPurchaseToken(purchaseToken)
            .build()
        
        billingClient.acknowledgePurchase(acknowledgePurchaseParams) { billingResult ->
            if (billingResult.responseCode == BillingClient.BillingResponseCode.OK) {
                invoke.resolve(JSObject().put("success", true))
            } else {
                invoke.reject("Failed to acknowledge purchase: ${billingResult.debugMessage}")
            }
        }
    }
    
    @Command
    fun getProductStatus(invoke: Invoke) {
        val args = invoke.parseArgs(GetProductStatusArgs::class.java)
        
        if (!billingClient.isReady) {
            invoke.reject("Billing client not ready")
            return
        }
        
        val productType = translateProductType(args.productType)
        
        val params = QueryPurchasesParams.newBuilder()
            .setProductType(productType)
            .build()
        
        billingClient.queryPurchasesAsync(params) { billingResult, purchases ->
            if (billingResult.responseCode == BillingClient.BillingResponseCode.OK) {
                val productPurchase = purchases.find { purchase ->
                    purchase.products.contains(args.productId)
                }
                
                val statusResult = JSObject().apply {
                    put("productId", args.productId)
                    
                    if (productPurchase != null) {
                        put("isOwned", true)
                        put("purchaseState", translatePurchaseState(productPurchase.purchaseState))
                        put("purchaseTime", productPurchase.purchaseTime)
                        put("isAutoRenewing", productPurchase.isAutoRenewing)
                        put("isAcknowledged", productPurchase.isAcknowledged)
                        put("purchaseToken", productPurchase.purchaseToken)
                        
                        // Note: Android doesn't provide expiration time directly for subscriptions
                        // It would require additional Google Play Developer API calls
                    } else {
                        put("isOwned", false)
                    }
                }
                
                invoke.resolve(statusResult)
            } else {
                invoke.reject("Failed to get product status: ${billingResult.debugMessage}")
            }
        }
    }
    
    override fun onPurchasesUpdated(billingResult: BillingResult, purchases: List<Purchase>?) {
        when (billingResult.responseCode) {
            BillingClient.BillingResponseCode.OK -> {
                purchases?.let { purchaseList ->
                    for (purchase in purchaseList) {
                        handlePurchase(purchase)
                    }
                }
            }
            BillingClient.BillingResponseCode.USER_CANCELED -> {
                pendingPurchaseInvoke?.reject("Purchase cancelled by user")
                pendingPurchaseInvoke = null
            }
            else -> {
                pendingPurchaseInvoke?.reject("Purchase failed: ${billingResult.debugMessage}")
                pendingPurchaseInvoke = null
            }
        }
    }
    
    private fun handlePurchase(purchase: Purchase) {
        if (purchase.purchaseState == Purchase.PurchaseState.PURCHASED) {
            val purchaseData = JSObject().apply {
                put("orderId", purchase.orderId)
                put("packageName", purchase.packageName)
                put("productId", purchase.products.firstOrNull() ?: "")
                put("purchaseTime", purchase.purchaseTime)
                put("purchaseToken", purchase.purchaseToken)
                put("purchaseState", translatePurchaseState(purchase.purchaseState))
                put("isAutoRenewing", purchase.isAutoRenewing)
                put("isAcknowledged", purchase.isAcknowledged)
                put("originalJson", purchase.originalJson)
                put("signature", purchase.signature)
            }

            pendingPurchaseInvoke?.resolve(purchaseData)
            pendingPurchaseInvoke = null
            
            // Emit event for purchase state change
            trigger("purchaseUpdated", purchaseData)
        }
    }
    
    override fun onBillingSetupFinished(billingResult: BillingResult) {
        if (billingResult.responseCode == BillingClient.BillingResponseCode.OK) {
            Logger.debug(TAG, "Billing setup finished successfully")
        }
    }

    override fun onBillingServiceDisconnected() {
        Logger.debug(TAG, "Billing service disconnected")
    }
}