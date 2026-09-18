import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// LVC Seedance — Vite config cho frontend Tauri 2 (app/). Quy uoc theo
// `crates/lvc-app/tauri.conf.json`: devUrl http://localhost:5173 (strictPort — Tauri
// khong tu doi cong), frontendDist ../../app/dist, beforeDevCommand/beforeBuildCommand
// da tro vao `pnpm -C app dev|build`.
export default defineConfig({
  plugins: [react(), tailwindcss()],

  // Tauri mong doi mot cong co dinh cho dev server.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  // Bien moi truong Tauri bom vao (vd TAURI_PLATFORM) phai duoc Vite lo ra ngoai
  // cung voi tien to VITE_ mac dinh.
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    outDir: "dist",
  },

  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
  },
});
