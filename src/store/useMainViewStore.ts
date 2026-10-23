import { create } from "zustand";
import { api } from "../api/tauri";

// 右ゾーンで表示している画面(詩織Ver3.9、ウィンドウとDockをやめてタブ切り替えにした)。
// 永続化はせず、起動のたびにホームから始める。
export type MainView =
  | "home"
  | "knowledge"
  | "library"
  | "library-browse"
  | "shelf"
  | "rules"
  | "claude"
  | "settings";

interface MainViewStore {
  active: MainView;
  setActive: (view: MainView) => void;
  // ホームの検索欄から図書館へ渡す検索語。図書館が受け取ったらnullに戻す。
  libraryQuery: string | null;
  searchInLibrary: (query: string) => void;
  clearLibraryQuery: () => void;
}

// タブの移動をログに残す。記録に失敗しても、画面の切り替えは止めない。
function logMove(from: MainView, to: MainView) {
  if (from !== to) {
    api.logUiEvent("タブを移った", { from, to }).catch(() => undefined);
  }
}

export const useMainViewStore = create<MainViewStore>((set, get) => ({
  active: "home",
  setActive: (view) => {
    logMove(get().active, view);
    set({ active: view });
  },
  libraryQuery: null,
  searchInLibrary: (query) => {
    logMove(get().active, "library");
    set({ active: "library", libraryQuery: query });
  },
  clearLibraryQuery: () => set({ libraryQuery: null }),
}));
