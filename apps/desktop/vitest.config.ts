import { defineConfig } from "vite";

export default defineConfig({
  test: {
    environment: "node",
    globals: true,
    include: ["src/**/*.test.ts"],
    exclude: ["src/**/*.svelte.test.ts", "**/node_modules/**", "**/.git/**"],
    clearMocks: true,
    restoreMocks: true,
    testTimeout: 20000,
  },
});
