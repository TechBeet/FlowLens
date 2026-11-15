// FlowLens — Vitest configuration.
//
// The filter/sort/stats composables under test are pure functions with no DOM
// dependencies, so `environment: "node"` is sufficient there. The event store
// (`eventStore.svelte.ts`) uses Svelte 5 runes, so the Svelte plugin must be
// registered for `.svelte.ts` modules to be compiled with the runes transform.
import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte({ hot: false })],
  resolve: {
    conditions: ["browser"],
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
    globals: false,
  },
});