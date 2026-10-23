import { create } from "zustand";
import { api } from "../api/tauri";
import type { ExtensionInfo } from "../types";

// 拡張機能の一覧(詩織Ver6.0の段階1)。拡張機能のタブと、起動時の承認の画面で同じ一覧を使う。
interface ExtensionsStore {
  list: ExtensionInfo[] | null;
  error: string | null;
  // 承認の画面で「今回は読み込まない」を選んだ拡張機能(この起動の間だけ、聞き直さない)
  skipped: string[];
  load: () => Promise<void>;
  approve: (name: string) => Promise<void>;
  skip: (name: string) => void;
}

export const useExtensionsStore = create<ExtensionsStore>((set) => ({
  list: null,
  error: null,
  skipped: [],
  load: async () => {
    try {
      set({ list: await api.listExtensions(), error: null });
    } catch (err) {
      set({ error: String(err) });
    }
  },
  approve: async (name) => {
    try {
      set({ list: await api.approveExtension(name), error: null });
    } catch (err) {
      set({ error: String(err) });
    }
  },
  skip: (name) => set((s) => ({ skipped: [...s.skipped, name] })),
}));
