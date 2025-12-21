<script>
  import { onMount, onDestroy } from 'svelte'
  import {
    initialize,
    getProducts,
    purchase,
    restorePurchases,
    getProductStatus,
    acknowledgePurchase,
    onPurchaseUpdated
  } from 'tauri-plugin-iap-api'

	let response = ''
	let isInitialized = false
	let products = []
	let productIds = ''
	let productType = 'inapp'
	let selectedProductId = ''
	let purchaseHistory = []
	let statusProductId = ''
	let productStatus = null
	let purchaseListener = null
	let listenerReady = false

	function updateResponse(returnValue) {
		response += `[${new Date().toLocaleTimeString()}] ` + (typeof returnValue === 'string' ? returnValue : JSON.stringify(returnValue, null, 2)) + '\n\n'
	}

	// Setup purchase listener on mount
	onMount(async () => {
		console.log('[IAP Demo] Setting up purchase update listener...')
		updateResponse('⏳ Setting up purchase update listener...')
		try {
			purchaseListener = await onPurchaseUpdated((purchase) => {
				console.log('[IAP Demo] Purchase update received:', purchase)
				updateResponse('🔔 Purchase updated: ' + JSON.stringify(purchase, null, 2))
			})
			listenerReady = true
			console.log('[IAP Demo] Purchase listener registered successfully')
			updateResponse('✓ Purchase update listener ready')
		} catch (error) {
			console.error('[IAP Demo] Failed to setup purchase listener:', error)
			updateResponse('✗ Failed to setup purchase listener: ' + JSON.stringify(error))
		}
	})

	// Cleanup listener on destroy
	onDestroy(async () => {
		if (purchaseListener) {
			console.log('[IAP Demo] Cleaning up purchase listener...')
			try {
				await purchaseListener.unregister()
				console.log('[IAP Demo] Purchase listener unregistered')
			} catch (error) {
				console.error('[IAP Demo] Error unregistering listener:', error)
			}
		}
	})

	async function handleInitialize() {
		try {
			await initialize()
			isInitialized = true
			updateResponse('✓ IAP initialized successfully')
		} catch (error) {
			updateResponse('✗ Initialize failed: ' + JSON.stringify(error))
		}
	}

	async function handleGetProducts() {
		if (!productIds.trim()) {
			updateResponse('✗ Please enter product IDs')
			return
		}
		try {
			const ids = productIds.split(',').map(id => id.trim()).filter(id => id)
			const result = await getProducts(ids, productType)
			products = result
			updateResponse('✓ Products fetched: ' + JSON.stringify(result, null, 2))
		} catch (error) {
			updateResponse('✗ Get products failed: ' + JSON.stringify(error))
		}
	}

	async function handlePurchase() {
		if (!selectedProductId.trim()) {
			updateResponse('✗ Please enter a product ID')
			return
		}
		try {
			const result = await purchase(selectedProductId, productType)
			updateResponse('✓ Purchase initiated: ' + JSON.stringify(result, null, 2))
		} catch (error) {
			updateResponse('✗ Purchase failed: ' + JSON.stringify(error))
		}
	}

	async function handleRestorePurchases() {
		try {
			const result = await restorePurchases(productType)
			purchaseHistory = result
			updateResponse('✓ Purchases restored: ' + JSON.stringify(result, null, 2))
		} catch (error) {
			updateResponse('✗ Restore purchases failed: ' + JSON.stringify(error))
		}
	}

	async function handleGetProductStatus() {
		if (!statusProductId.trim()) {
			updateResponse('✗ Please enter a product ID')
			return
		}
		try {
			const result = await getProductStatus(statusProductId, productType)
			productStatus = result
			updateResponse('✓ Product status: ' + JSON.stringify(result, null, 2))
		} catch (error) {
			updateResponse('✗ Get product status failed: ' + JSON.stringify(error))
		}
	}
</script>

<main class="container">
  <h1>Tauri IAP Plugin Demo</h1>

  <div class="info-box">
    <h3>About this demo</h3>
    <p>This example demonstrates the core functionality of the Tauri In-App Purchase plugin.</p>
    <p><strong>Supported platforms:</strong> iOS, Android, macOS, Windows</p>
    <p><strong>Note:</strong> You'll need to configure products in your app store accounts to test purchases.</p>
  </div>

  <div class="section">
    <h2>1. Initialize IAP</h2>
    <p class="doc">Initialize the IAP connection. Must be called before any other operations.</p>
    <button on:click={handleInitialize} disabled={isInitialized}>
      {isInitialized ? '✓ Initialized' : 'Initialize IAP'}
    </button>
  </div>

  <div class="section">
    <h2>2. Get Products</h2>
    <p class="doc">Fetch product information from the store. Enter product IDs separated by commas.</p>
    <div class="form-group">
      <label>
        Product IDs:
        <input
          type="text"
          bind:value={productIds}
          placeholder="com.example.product1, com.example.product2"
          disabled={!isInitialized}
        />
      </label>
      <label>
        Product Type:
        <select bind:value={productType} disabled={!isInitialized}>
          <option value="inapp">In-App Purchase</option>
          <option value="subs">Subscription</option>
        </select>
      </label>
      <button on:click={handleGetProducts} disabled={!isInitialized}>Get Products</button>
    </div>
    {#if products.length > 0}
      <div class="products-list">
        <h4>Available Products:</h4>
        {#each products as product}
          <div class="product-card">
            <strong>{product.title}</strong>
            <p>{product.description}</p>
            <p class="price">{product.formattedPrice || 'Price not available'}</p>
            <code>ID: {product.productId}</code>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div class="section">
    <h2>3. Purchase Product</h2>
    <p class="doc">Initiate a purchase for a specific product. Enter the product ID to purchase.</p>
    <div class="form-group">
      <label>
        Product ID:
        <input
          type="text"
          bind:value={selectedProductId}
          placeholder="com.example.product"
          disabled={!isInitialized}
        />
      </label>
      <button on:click={handlePurchase} disabled={!isInitialized}>Purchase</button>
    </div>
    <p class="note"><strong>Android:</strong> For subscriptions, you may need to provide an offer token.</p>
    <p class="note"><strong>iOS:</strong> Optionally provide an appAccountToken (UUID) for user tracking.</p>
  </div>

  <div class="section">
    <h2>4. Restore Purchases</h2>
    <p class="doc">Restore previously purchased products. Useful for transferring purchases to a new device.</p>
    <button on:click={handleRestorePurchases} disabled={!isInitialized}>Restore Purchases</button>
    {#if purchaseHistory.length > 0}
      <div class="purchases-list">
        <h4>Purchase History:</h4>
        {#each purchaseHistory as purchase}
          <div class="purchase-card">
            <strong>Product: {purchase.productId}</strong>
            <p>Order ID: {purchase.orderId || 'N/A'}</p>
            <p>State: {purchase.purchaseState === 0 ? 'Purchased' : purchase.purchaseState === 1 ? 'Canceled' : 'Pending'}</p>
            <p>Auto-renewing: {purchase.isAutoRenewing ? 'Yes' : 'No'}</p>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div class="section">
    <h2>5. Check Product Status</h2>
    <p class="doc">Check if the user owns a specific product and get subscription details.</p>
    <div class="form-group">
      <label>
        Product ID:
        <input
          type="text"
          bind:value={statusProductId}
          placeholder="com.example.product"
          disabled={!isInitialized}
        />
      </label>
      <button on:click={handleGetProductStatus} disabled={!isInitialized}>Check Status</button>
    </div>
    {#if productStatus}
      <div class="status-card">
        <h4>Product Status:</h4>
        <p><strong>Owned:</strong> {productStatus.isOwned ? 'Yes ✓' : 'No ✗'}</p>
        {#if productStatus.isOwned}
          <p><strong>Purchase Time:</strong> {new Date(productStatus.purchaseTime).toLocaleString()}</p>
          {#if productStatus.expirationTime}
            <p><strong>Expires:</strong> {new Date(productStatus.expirationTime).toLocaleString()}</p>
          {/if}
          <p><strong>Auto-renewing:</strong> {productStatus.isAutoRenewing ? 'Yes' : 'No'}</p>
        {/if}
      </div>
    {/if}
  </div>

  <div class="section">
    <h2>Response Log</h2>
    <p class="doc">All API responses and purchase updates will appear here.</p>
    <pre class="response-box">{response || 'No activity yet...'}</pre>
    <button on:click={() => response = ''}>Clear Log</button>
  </div>

</main>

<style>
  :global(body) {
    background: #1a1a1a;
    color: #e0e0e0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
  }

  .container {
    max-width: 800px;
    margin: 0 auto;
    padding: 30px 20px;
  }

  h1 {
    text-align: center;
    color: #ffffff;
    margin-bottom: 35px;
    font-size: 2em;
    font-weight: 600;
  }

  h2 {
    color: #2196f3;
    margin-top: 0;
    margin-bottom: 12px;
    font-size: 1.25em;
    font-weight: 600;
  }

  .info-box {
    background: linear-gradient(135deg, #1e3c72 0%, #2a5298 100%);
    border: 1px solid #2196f3;
    padding: 20px;
    margin-bottom: 30px;
    border-radius: 8px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.3);
  }

  .info-box h3 {
    margin-top: 0;
    margin-bottom: 12px;
    color: #64b5f6;
    font-size: 1.3em;
    font-weight: 600;
  }

  .info-box p {
    margin: 8px 0;
    color: #e3f2fd;
    line-height: 1.5;
  }

  .section {
    background: #242424;
    border: 1px solid #333;
    border-radius: 8px;
    padding: 24px;
    margin-bottom: 20px;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  }

  .doc {
    color: #999;
    font-style: italic;
    margin: 10px 0 15px 0;
    font-size: 0.95em;
    line-height: 1.4;
  }

  .note {
    font-size: 0.85em;
    color: #ffa726;
    margin: 8px 0;
    padding: 8px 12px;
    background: rgba(255, 167, 38, 0.1);
    border-left: 3px solid #ffa726;
    border-radius: 3px;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 15px;
    margin: 15px 0;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-weight: 500;
    color: #bbb;
    font-size: 0.95em;
  }

  input, select {
    padding: 10px 14px;
    border: 1px solid #444;
    border-radius: 6px;
    font-size: 14px;
    background: #1a1a1a;
    color: #e0e0e0;
    transition: border-color 0.2s, background 0.2s;
  }

  input:focus, select:focus {
    outline: none;
    border-color: #2196f3;
    background: #222;
  }

  input:disabled, select:disabled, button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  button {
    background: linear-gradient(135deg, #2196f3 0%, #1976d2 100%);
    color: white;
    border: none;
    padding: 12px 24px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
    font-weight: 600;
    transition: transform 0.1s, box-shadow 0.2s;
    box-shadow: 0 2px 4px rgba(33, 150, 243, 0.3);
  }

  button:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 4px 8px rgba(33, 150, 243, 0.4);
  }

  button:active:not(:disabled) {
    transform: translateY(0);
  }

  .products-list, .purchases-list {
    margin-top: 15px;
  }

  .product-card, .purchase-card, .status-card {
    background: #1a1a1a;
    border: 1px solid #444;
    border-radius: 8px;
    padding: 16px;
    margin: 12px 0;
    transition: border-color 0.2s, transform 0.2s;
  }

  .product-card:hover, .purchase-card:hover {
    border-color: #2196f3;
    transform: translateX(4px);
  }

  .product-card strong, .purchase-card strong {
    display: block;
    color: #ffffff;
    margin-bottom: 10px;
    font-size: 1.1em;
    font-weight: 600;
  }

  .product-card p, .purchase-card p, .status-card p {
    margin: 6px 0;
    color: #bbb;
    font-size: 0.95em;
  }

  .products-list h4, .purchases-list h4 {
    color: #64b5f6;
    margin-bottom: 12px;
    font-size: 1.1em;
  }

  .price {
    color: #66bb6a;
    font-weight: bold;
    font-size: 1.3em;
    margin: 8px 0;
  }

  code {
    background: #333;
    color: #64b5f6;
    padding: 3px 8px;
    border-radius: 4px;
    font-size: 0.85em;
    font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
  }

  .response-box {
    background: #0d1117;
    color: #c9d1d9;
    padding: 16px;
    border-radius: 6px;
    border: 1px solid #30363d;
    max-height: 400px;
    overflow-y: auto;
    font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
    font-size: 13px;
    white-space: pre-wrap;
    word-wrap: break-word;
    line-height: 1.5;
  }

  .status-card h4 {
    margin-top: 0;
    margin-bottom: 12px;
    color: #64b5f6;
    font-size: 1.1em;
  }

  .status-card {
    background: #1e1e1e;
  }
</style>
