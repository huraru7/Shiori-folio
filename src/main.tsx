import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { api } from "./api/tauri";
import "./styles/tokens.css";

// 画面側の未処理のエラーを、ログ(data/logs/)に残す(詩織Ver4.0)。
window.addEventListener("error", (e) => {
  api.logFrontendError(`${e.message} (${e.filename}:${e.lineno})`).catch(() => undefined);
});
window.addEventListener("unhandledrejection", (e) => {
  api.logFrontendError(`処理されなかったPromiseの失敗: ${String(e.reason)}`).catch(() => undefined);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
