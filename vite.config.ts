import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vite.dev/config/
export default defineConfig({
  build: {
    outDir: "src/diffodil/statics",
  },
  server: {
    proxy: {
      "/ws": {
        target: "ws://localhost:8765",
        ws: true,
        rewriteWsOrigin: true,
      },
    },
  },

  plugins: [react()],
});
