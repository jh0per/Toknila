import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The daemon embeds dist/ into its binary and serves it at /.
// `npm run dev` proxies the API to a running daemon: TOKNILA_PORT=<port from daemon.json>.
const port = process.env.TOKNILA_PORT ?? "0";
export default defineConfig({
  plugins: [react()],
  base: "/",
  build: { outDir: "dist", emptyOutDir: true, assetsDir: "assets" },
  server: {
    proxy: {
      "/v1": { target: `http://127.0.0.1:${port}`, ws: true },
    },
  },
});
