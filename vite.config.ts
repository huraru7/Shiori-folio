import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri` and `third_party`
      // (third_partyはllama.cpp/whisper.cpp/piper-plus等のエンジンを丸ごとcloneしたもので、
      // Vite側のプロジェクトとは無関係。含めたままだとpiper-plusのWASM向けソースが依存
      // スキャンに誤って巻き込まれ、`@piper-plus/g2p`解決失敗の警告が出る)
      ignored: ["**/src-tauri/**", "**/third_party/**"],
    },
  },
}));
