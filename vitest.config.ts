import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    environment: "happy-dom",
    include: ["guest-js/**/*.{test,spec}.{js,ts}"],
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      include: ["guest-js/**/*.ts"],
      exclude: ["guest-js/**/*.{test,spec}.ts"],
    },
  },
});
