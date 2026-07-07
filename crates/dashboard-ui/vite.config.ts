import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// During development, the UI runs on http://localhost:5173 and the dashboard
// service runs on http://localhost:8081. The browser would normally treat
// these as different origins (CORS + cookie issues), so we proxy the
// dashboard API through Vite's dev server. Production builds are served
// from a static host and rely on the dashboard service's CORS headers
// (or the same-origin pattern of being served from one place).
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/dashboard": {
        target: "http://localhost:8081",
        changeOrigin: false,
      },
    },
  },
  build: {
    outDir: "dist",
    sourcemap: true,
  },
});
