import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const tauriDevHost = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  server: {
    host: tauriDevHost || "0.0.0.0",
    port: 1420,
    strictPort: true,
    hmr: tauriDevHost
      ? {
        protocol: "ws",
        host: tauriDevHost,
        port: 1421,
        clientPort: 1421,
      }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
