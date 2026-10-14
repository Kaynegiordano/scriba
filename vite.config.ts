import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/data/**"] } },
  // Vite 8 minifie avec Oxc par defaut (esbuild n'est plus fourni)
  build: { target: "es2021", minify: true, sourcemap: false },
});
