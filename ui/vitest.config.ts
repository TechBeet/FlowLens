// FlowLens — Vitest configuration.
//
// The default `vite.config.mjs` does not set `test`, so Vitest inherits its
// own defaults here. `environment: "node"` is sufficient because the filter,
// sort, and stats composables under test are pure functions with no DOM
// dependencies (the runes-based store itself is covered separately).
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
    globals: false,
  },
});