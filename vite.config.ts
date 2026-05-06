import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

// Tauri expects Vite to:
//   - serve on a fixed port (so the Rust side knows the dev URL),
//   - skip clearing the terminal (Tauri owns terminal output),
//   - listen on the LAN-bound interface during mobile dev (irrelevant for Klipo v0.1).
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Don't trigger Vite reload on Rust/bench/docs changes.
      ignored: ["**/src-tauri/**", "**/bench/**", "**/docs/**"],
    },
  },
  build: {
    target: "esnext",
    minify: "esbuild",
    sourcemap: false,
    chunkSizeWarningLimit: 1000,
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
});
