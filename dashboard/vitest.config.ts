import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Component-level unit tests: render client components in jsdom with a mocked
// `@/lib/api`, no browser and no running server. The heavier browser-driven
// flows live in the Playwright `e2e` project instead.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": fileURLToPath(new URL(".", import.meta.url)) },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./tests/unit/setup.ts"],
    include: ["tests/unit/**/*.test.{ts,tsx}"],
  },
});
