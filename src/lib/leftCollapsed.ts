// 左ゾーン(会話UI)の折りたたみ状態(詩織Ver3.0、UI改善4-3節)。
// useWindowStoreはセッション内メモリのみで非永続だが、この状態は次回
// アプリ起動時にも維持したいためlocalStorageに保存する(Tauriのwebviewは
// ローカルにプロファイルを保持するため、通常のブラウザと同様に永続化される)。
const STORAGE_KEY = "shiori.leftCollapsed";

export function getLeftCollapsed(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "true";
  } catch {
    return false;
  }
}

export function setLeftCollapsed(collapsed: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, String(collapsed));
  } catch {
    // localStorageが使えない環境でも、折りたたみ操作自体は継続できるよう無視する。
  }
}
