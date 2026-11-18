import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri expects a fixed dev port and does not want Vite to clear the terminal.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  // The program has two surfaces: the window and the overlay over the game.
  build: {
    rollupOptions: {
      input: {
        main: "index.html",
        overlay: "overlay.html",
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
