import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  // Prevent vite from obscuring Rust errors
  clearScreen: false,
  server: {
    // Tauri expects a fixed port, fail if unavailable
    port: 1420,
    strictPort: true,
    watch: {
      // Tell vite to ignore src-tauri (Rust rebuild is handled by tauri-cli)
      ignored: ["**/src-tauri/**"],
    },
  },
});
