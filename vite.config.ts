import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Fixed port: src-tauri/tauri.conf.json (build.devUrl) points at it.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: { target: "es2020", sourcemap: false },
});
