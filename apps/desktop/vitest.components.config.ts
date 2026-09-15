import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    conditions: ["browser"],
  },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.svelte.test.ts"],
    setupFiles: ["./src/test/setup.ts"],
    testTimeout: 20000,
    clearMocks: true,
    restoreMocks: true,
    fileParallelism: false,
  },
});
