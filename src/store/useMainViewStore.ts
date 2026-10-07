import { create } from "zustand";

// 右ゾーンで表示している画面(詩織Ver3.9、ウィンドウとDockをやめてタブ切り替えにした)。
// 永続化はせず、起動のたびにホームから始める。
export type MainView = "home" | "knowledge" | "library" | "library-browse" | "claude" | "settings";

interface MainViewStore {
  active: MainView;
  setActive: (view: MainView) => void;
  // ホームの検索欄から図書館へ渡す検索語。図書館が受け取ったらnullに戻す。
  libraryQuery: string | null;
  searchInLibrary: (query: string) => void;
  clearLibraryQuery: () => void;
}

export const useMainViewStore = create<MainViewStore>((set) => ({
  active: "home",
  setActive: (view) => set({ active: view }),
  libraryQuery: null,
  searchInLibrary: (query) => set({ active: "library", libraryQuery: query }),
  clearLibraryQuery: () => set({ libraryQuery: null }),
}));
