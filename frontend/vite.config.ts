import { defineConfig } from 'vite'
import {resolve} from "path";
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from "@tailwindcss/vite";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [svelte(), tailwindcss()],
  resolve: {
    alias: {
      $lib: resolve("./src/lib"),
      $common: resolve("./src/common.ts"),
      $ui: resolve("./src/lib/components/ui")
    },
  },
})
