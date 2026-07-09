import { defineConfig } from "vitest/config";

// The live-preview core (activeRegions, decoration building, Lezer parsers) is
// pure @codemirror/state + @lezer — no DOM needed, so tests run in plain node.
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
