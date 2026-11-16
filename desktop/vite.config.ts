import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed port and no screen clearing (its own logs share the terminal)
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  // the dev server may read docs/WIDGETS.md (creator mode sends it to the agent) from the repository above
  server: { port: 1420, strictPort: true, fs: { allow: [".."] } },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: { target: "es2022", outDir: "dist" },
});
