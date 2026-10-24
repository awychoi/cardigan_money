import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

// Local-only dev server. Production CSP has no remote source; the dev CSP only
// allows the loopback Vite server and its HMR websocket.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      // The real mock bridge exists only in development/test builds.
      "@mock-bridge": path.resolve(
        __dirname,
        process.env.NODE_ENV === "production" ? "src/mocks/bridge.disabled.ts" : "src/mocks/bridge.ts",
      ),
    },
  },
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    hmr: { host: "127.0.0.1", port: 1420 },
  },
  build: {
    target: "safari15",
    sourcemap: false,
    minify: "esbuild",
  },
});
