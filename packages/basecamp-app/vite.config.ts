import { defineConfig } from "vite";

export default defineConfig({
  root: ".",
  build: {
    outDir: "dist",
    target: "es2022",
    sourcemap: true,
  },
  server: {
    port: 4172,
    host: "127.0.0.1",
  },
});
