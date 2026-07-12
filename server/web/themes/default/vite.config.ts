import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

// Base is relative so the built theme works no matter where the Ichoi server
// mounts it (root, or a path-scoped `/themes/default/`, §11). `import.meta.env.BASE_URL`
// resolves the AudioWorklet module URL at runtime against this same base.
export default defineConfig({
  base: "./",
  plugins: [solid()],
  build: {
    target: "es2022",
    outDir: "dist",
    sourcemap: true,
  },
  server: {
    port: 5173,
    // During `npm run dev` we proxy the server's WebSocket carrier so the SPA can
    // talk to a locally running Ichoi core without CORS/again-origin friction.
    proxy: {
      "/ws": {
        target: "http://localhost:4042",
        ws: true,
        changeOrigin: true,
      },
    },
  },
});
