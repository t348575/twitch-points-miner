import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    // The Rust backend serves ./dist relative to its working directory
    // (see app/src/web_api/mod.rs, ServeDir::new("dist")).
    // echarts is intentionally a large, separately cached vendor chunk.
    chunkSizeWarningLimit: 700,
    outDir: "../dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        // Split the heavy vendors so they cache independently of app code.
        // Matching on the resolved path keeps this to what is actually imported:
        // naming the "echarts" barrel here would drag in every chart type.
        manualChunks(id) {
          if (!id.includes("node_modules")) return undefined;
          if (id.includes("node_modules/echarts") || id.includes("node_modules/zrender")) {
            return "echarts";
          }
          if (id.includes("node_modules/@mantine")) return "mantine";
          if (/node_modules\/(react|react-dom|react-router)/.test(id)) return "react";
          return undefined;
        },
      },
    },
  },
  server: {
    // Bind to all hosts so the devcontainer port forward can reach it
    host: true,
    port: 5173,
  },
});
