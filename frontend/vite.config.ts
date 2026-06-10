import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// During local `vite dev`, proxy API calls to the backend container/host.
// In production the build is served statically by nginx, which proxies /api.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: process.env.VITE_API_TARGET ?? "http://localhost:8080",
        changeOrigin: true,
      },
    },
  },
});
