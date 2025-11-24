import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  initialize,
  getProducts,
  purchase,
  restorePurchases,
  getPurchaseHistory,
  acknowledgePurchase,
  getProductStatus,
  onPurchaseUpdated,
  PurchaseState,
  type InitializeResponse,
  type GetProductsResponse,
  type Purchase,
  type RestorePurchasesResponse,
  type GetPurchaseHistoryResponse,
  type AcknowledgePurchaseResponse,
  type ProductStatus,
  type PurchaseOptions,
} from "./index";

// Mock Tauri API
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

describe("IAP Plugin", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("initialize", () => {
    it("should call invoke with correct command", async () => {
      const mockResponse: InitializeResponse = { success: true };
      vi.mocked(invoke).mockResolvedValue(mockResponse);

      const result = await initialize();

      expect(invoke).toHaveBeenCalledWith("plugin:iap|initialize");
      expect(result).toEqual(mockResponse);
    });

    it("should handle initialization failure", async () => {
      const mockResponse: InitializeResponse = { success: false };
      vi.mocked(invoke).mockResolvedValue(mockResponse);

      const result = await initialize();

      expect(result.success).toBe(false);
    });

    it("should propagate errors from invoke", async () => {
      const error = new Error("Initialization failed");
      vi.mocked(invoke).mockRejectedValue(error);

      await expect(initialize()).rejects.toThrow("Initialization failed");
    });
  });

  describe("getProducts", () => {
    it("should fetch subscription products with correct parameters", async () => {
      const mockProducts: GetProductsResponse = {
        products: [
          {
            productId: "com.example.premium",
            title: "Premium Subscription",
            description: "Premium features",
            productType: "subs",
            formattedPrice: "$9.99",
            priceCurrencyCode: "USD",
            priceAmountMicros: 9990000,
          },
        ],
      };
      vi.mocked(invoke).mockResolvedValue(mockProducts);

      const result = await getProducts(["com.example.premium"], "subs");

      expect(invoke).toHaveBeenCalledWith("plugin:iap|get_products", {
        payload: {
          productIds: ["com.example.premium"],
          productType: "subs",
        },
      });
      expect(result).toEqual(mockProducts);
    });

    it("should default to subs product type", async () => {
      const mockProducts: GetProductsResponse = { products: [] };
      vi.mocked(invoke).mockResolvedValue(mockProducts);

      await getProducts(["com.example.product"]);

      expect(invoke).toHaveBeenCalledWith("plugin:iap|get_products", {
        payload: {
          productIds: ["com.example.product"],
          productType: "subs",
        },
      });
    });

    it("should fetch in-app products", async () => {
      const mockProducts: GetProductsResponse = {
        products: [
          {
            productId: "com.example.coins",
            title: "100 Coins",
            description: "In-game currency",
            productType: "inapp",
            formattedPrice: "$0.99",
          },
        ],
      };
      vi.mocked(invoke).mockResolvedValue(mockProducts);

      const result = await getProducts(["com.example.coins"], "inapp");

      expect(result.products[0].productType).toBe("inapp");
    });

    it("should handle empty product list", async () => {
      const mockProducts: GetProductsResponse = { products: [] };
      vi.mocked(invoke).mockResolvedValue(mockProducts);

      const result = await getProducts([]);

      expect(result.products).toHaveLength(0);
    });
  });

  describe("purchase", () => {
    it("should initiate purchase with correct parameters", async () => {
      const mockPurchase: Purchase = {
        orderId: "ORDER123",
        packageName: "com.example.app",
        productId: "com.example.premium",
        purchaseTime: Date.now(),
        purchaseToken: "TOKEN123",
        purchaseState: PurchaseState.PURCHASED,
        isAutoRenewing: true,
        isAcknowledged: false,
        originalJson: "{}",
        signature: "SIG123",
      };
      vi.mocked(invoke).mockResolvedValue(mockPurchase);

      const result = await purchase("com.example.premium", "subs");

      expect(invoke).toHaveBeenCalledWith("plugin:iap|purchase", {
        payload: {
          productId: "com.example.premium",
          productType: "subs",
        },
      });
      expect(result).toEqual(mockPurchase);
    });

    it("should include purchase options in payload", async () => {
      const mockPurchase: Purchase = {
        packageName: "com.example.app",
        productId: "com.example.premium",
        purchaseTime: Date.now(),
        purchaseToken: "TOKEN123",
        purchaseState: PurchaseState.PURCHASED,
        isAutoRenewing: false,
        isAcknowledged: true,
        originalJson: "{}",
        signature: "SIG123",
      };
      vi.mocked(invoke).mockResolvedValue(mockPurchase);

      const options: PurchaseOptions = {
        offerToken: "OFFER123",
        obfuscatedAccountId: "ACC123",
        obfuscatedProfileId: "PROF123",
      };

      await purchase("com.example.premium", "subs", options);

      expect(invoke).toHaveBeenCalledWith("plugin:iap|purchase", {
        payload: {
          productId: "com.example.premium",
          productType: "subs",
          offerToken: "OFFER123",
          obfuscatedAccountId: "ACC123",
          obfuscatedProfileId: "PROF123",
        },
      });
    });

    it("should handle iOS app account token", async () => {
      const mockPurchase: Purchase = {
        packageName: "com.example.app",
        productId: "com.example.premium",
        purchaseTime: Date.now(),
        purchaseToken: "TOKEN123",
        purchaseState: PurchaseState.PURCHASED,
        isAutoRenewing: true,
        isAcknowledged: true,
        originalJson: "{}",
        signature: "SIG123",
      };
      vi.mocked(invoke).mockResolvedValue(mockPurchase);

      const options: PurchaseOptions = {
        appAccountToken: "550e8400-e29b-41d4-a716-446655440000",
      };

      await purchase("com.example.premium", "subs", options);

      expect(invoke).toHaveBeenCalledWith("plugin:iap|purchase", {
        payload: {
          productId: "com.example.premium",
          productType: "subs",
          appAccountToken: "550e8400-e29b-41d4-a716-446655440000",
        },
      });
    });

    it("should handle pending purchase state", async () => {
      const mockPurchase: Purchase = {
        packageName: "com.example.app",
        productId: "com.example.premium",
        purchaseTime: Date.now(),
        purchaseToken: "TOKEN123",
        purchaseState: PurchaseState.PENDING,
        isAutoRenewing: false,
        isAcknowledged: false,
        originalJson: "{}",
        signature: "SIG123",
      };
      vi.mocked(invoke).mockResolvedValue(mockPurchase);

      const result = await purchase("com.example.premium", "subs");

      expect(result.purchaseState).toBe(PurchaseState.PENDING);
    });
  });

  describe("restorePurchases", () => {
    it("should restore purchases with correct product type", async () => {
      const mockResponse: RestorePurchasesResponse = {
        purchases: [
          {
            orderId: "ORDER123",
            packageName: "com.example.app",
            productId: "com.example.premium",
            purchaseTime: Date.now(),
            purchaseToken: "TOKEN123",
            purchaseState: PurchaseState.PURCHASED,
            isAutoRenewing: true,
            isAcknowledged: true,
            originalJson: "{}",
            signature: "SIG123",
          },
        ],
      };
      vi.mocked(invoke).mockResolvedValue(mockResponse);

      const result = await restorePurchases("subs");

      expect(invoke).toHaveBeenCalledWith("plugin:iap|restore_purchases", {
        payload: {
          productType: "subs",
        },
      });
      expect(result).toEqual(mockResponse);
    });

    it("should default to subs product type", async () => {
      const mockResponse: RestorePurchasesResponse = { purchases: [] };
      vi.mocked(invoke).mockResolvedValue(mockResponse);

      await restorePurchases();

      expect(invoke).toHaveBeenCalledWith("plugin:iap|restore_purchases", {
        payload: {
          productType: "subs",
        },
      });
    });

    it("should handle empty purchase list", async () => {
      const mockResponse: RestorePurchasesResponse = { purchases: [] };
      vi.mocked(invoke).mockResolvedValue(mockResponse);

      const result = await restorePurchases("inapp");

      expect(result.purchases).toHaveLength(0);
    });
  });

  describe("getPurchaseHistory", () => {
    it("should fetch purchase history", async () => {
      const mockHistory: GetPurchaseHistoryResponse = {
        history: [
          {
            productId: "com.example.coins",
            purchaseTime: Date.now(),
            purchaseToken: "TOKEN123",
            quantity: 1,
            originalJson: "{}",
            signature: "SIG123",
          },
        ],
      };
      vi.mocked(invoke).mockResolvedValue(mockHistory);

      const result = await getPurchaseHistory();

      expect(invoke).toHaveBeenCalledWith("plugin:iap|get_purchase_history");
      expect(result).toEqual(mockHistory);
    });

    it("should handle empty history", async () => {
      const mockHistory: GetPurchaseHistoryResponse = { history: [] };
      vi.mocked(invoke).mockResolvedValue(mockHistory);

      const result = await getPurchaseHistory();

      expect(result.history).toHaveLength(0);
    });
  });

  describe("acknowledgePurchase", () => {
    it("should acknowledge purchase with token", async () => {
      const mockResponse: AcknowledgePurchaseResponse = { success: true };
      vi.mocked(invoke).mockResolvedValue(mockResponse);

      const result = await acknowledgePurchase("TOKEN123");

      expect(invoke).toHaveBeenCalledWith("plugin:iap|acknowledge_purchase", {
        payload: {
          purchaseToken: "TOKEN123",
        },
      });
      expect(result).toEqual(mockResponse);
    });

    it("should handle acknowledgment failure", async () => {
      const mockResponse: AcknowledgePurchaseResponse = { success: false };
      vi.mocked(invoke).mockResolvedValue(mockResponse);

      const result = await acknowledgePurchase("TOKEN123");

      expect(result.success).toBe(false);
    });
  });

  describe("getProductStatus", () => {
    it("should get product status with correct parameters", async () => {
      const mockStatus: ProductStatus = {
        productId: "com.example.premium",
        isOwned: true,
        purchaseState: PurchaseState.PURCHASED,
        purchaseTime: Date.now(),
        isAutoRenewing: true,
        isAcknowledged: true,
        purchaseToken: "TOKEN123",
      };
      vi.mocked(invoke).mockResolvedValue(mockStatus);

      const result = await getProductStatus("com.example.premium", "subs");

      expect(invoke).toHaveBeenCalledWith("plugin:iap|get_product_status", {
        payload: {
          productId: "com.example.premium",
          productType: "subs",
        },
      });
      expect(result).toEqual(mockStatus);
    });

    it("should default to subs product type", async () => {
      const mockStatus: ProductStatus = {
        productId: "com.example.premium",
        isOwned: false,
      };
      vi.mocked(invoke).mockResolvedValue(mockStatus);

      await getProductStatus("com.example.premium");

      expect(invoke).toHaveBeenCalledWith("plugin:iap|get_product_status", {
        payload: {
          productId: "com.example.premium",
          productType: "subs",
        },
      });
    });

    it("should indicate product not owned", async () => {
      const mockStatus: ProductStatus = {
        productId: "com.example.premium",
        isOwned: false,
      };
      vi.mocked(invoke).mockResolvedValue(mockStatus);

      const result = await getProductStatus("com.example.premium", "subs");

      expect(result.isOwned).toBe(false);
      expect(result.purchaseState).toBeUndefined();
    });

    it("should include expiration time for subscriptions", async () => {
      const now = Date.now();
      const mockStatus: ProductStatus = {
        productId: "com.example.premium",
        isOwned: true,
        purchaseState: PurchaseState.PURCHASED,
        purchaseTime: now,
        expirationTime: now + 30 * 24 * 60 * 60 * 1000, // 30 days
        isAutoRenewing: true,
      };
      vi.mocked(invoke).mockResolvedValue(mockStatus);

      const result = await getProductStatus("com.example.premium", "subs");

      expect(result.expirationTime).toBeDefined();
      expect(result.expirationTime).toBeGreaterThan(now);
    });
  });

  describe("onPurchaseUpdated", () => {
    it("should register event listener and return unsubscribe function", () => {
      const mockUnlisten = vi.fn();
      vi.mocked(listen).mockResolvedValue(mockUnlisten);

      const callback = vi.fn();
      const unsubscribe = onPurchaseUpdated(callback);

      expect(listen).toHaveBeenCalledWith(
        "purchaseUpdated",
        expect.any(Function),
      );
      expect(typeof unsubscribe).toBe("function");
    });

    it("should call callback with purchase data", async () => {
      const mockPurchase: Purchase = {
        orderId: "ORDER123",
        packageName: "com.example.app",
        productId: "com.example.premium",
        purchaseTime: Date.now(),
        purchaseToken: "TOKEN123",
        purchaseState: PurchaseState.PURCHASED,
        isAutoRenewing: true,
        isAcknowledged: false,
        originalJson: "{}",
        signature: "SIG123",
      };

      let eventCallback: ((event: { payload: Purchase }) => void) | undefined;
      vi.mocked(listen).mockImplementation((eventName, callback) => {
        eventCallback = callback as (event: { payload: Purchase }) => void;
        return Promise.resolve(vi.fn());
      });

      const callback = vi.fn();
      onPurchaseUpdated(callback);

      expect(eventCallback).toBeDefined();
      eventCallback!({ payload: mockPurchase });

      expect(callback).toHaveBeenCalledWith(mockPurchase);
    });

    it("should unsubscribe from event when cleanup function is called", async () => {
      const mockUnlisten = vi.fn();
      vi.mocked(listen).mockResolvedValue(mockUnlisten);

      const callback = vi.fn();
      const unsubscribe = onPurchaseUpdated(callback);

      await unsubscribe();

      await vi.waitFor(() => {
        expect(mockUnlisten).toHaveBeenCalled();
      });
    });

    it("should handle multiple purchase updates", async () => {
      let eventCallback: ((event: { payload: Purchase }) => void) | undefined;
      vi.mocked(listen).mockImplementation((eventName, callback) => {
        eventCallback = callback as (event: { payload: Purchase }) => void;
        return Promise.resolve(vi.fn());
      });

      const callback = vi.fn();
      onPurchaseUpdated(callback);

      const purchase1: Purchase = {
        packageName: "com.example.app",
        productId: "product1",
        purchaseTime: Date.now(),
        purchaseToken: "TOKEN1",
        purchaseState: PurchaseState.PENDING,
        isAutoRenewing: false,
        isAcknowledged: false,
        originalJson: "{}",
        signature: "SIG1",
      };

      const purchase2: Purchase = {
        packageName: "com.example.app",
        productId: "product1",
        purchaseTime: Date.now(),
        purchaseToken: "TOKEN1",
        purchaseState: PurchaseState.PURCHASED,
        isAutoRenewing: false,
        isAcknowledged: false,
        originalJson: "{}",
        signature: "SIG1",
      };

      eventCallback!({ payload: purchase1 });
      eventCallback!({ payload: purchase2 });

      expect(callback).toHaveBeenCalledTimes(2);
      expect(callback).toHaveBeenNthCalledWith(1, purchase1);
      expect(callback).toHaveBeenNthCalledWith(2, purchase2);
    });
  });

  describe("PurchaseState enum", () => {
    it("should have correct enum values", () => {
      expect(PurchaseState.PURCHASED).toBe(0);
      expect(PurchaseState.CANCELED).toBe(1);
      expect(PurchaseState.PENDING).toBe(2);
    });
  });
});
