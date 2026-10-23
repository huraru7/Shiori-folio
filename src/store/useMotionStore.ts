import { create } from "zustand";
import { api } from "../api/tauri";

// 水紋と星の動きを抑えるか(詩織Ver6.0の段階2)。config.jsonのui.reduceMotionに保存する。
// 設定していないときは、OSの「視差効果を減らす」(prefers-reduced-motion)に従う。
interface MotionStore {
  // config.jsonの値(未設定ならnull)
  configured: boolean | null;
  osReduce: boolean;
  loaded: boolean;
  load: () => Promise<void>;
  set: (reduce: boolean) => Promise<void>;
}

const media = typeof window !== "undefined" ? window.matchMedia("(prefers-reduced-motion: reduce)") : null;

export const useMotionStore = create<MotionStore>((set) => {
  media?.addEventListener("change", (e) => set({ osReduce: e.matches }));
  return {
    configured: null,
    osReduce: media?.matches ?? false,
    loaded: false,
    load: async () => {
      try {
        const config = await api.getConfig();
        set({ configured: config.reduceMotion, loaded: true });
      } catch {
        set({ loaded: true });
      }
    },
    set: async (reduce) => {
      set({ configured: reduce });
      await api.setConfig({ reduceMotion: reduce });
      api.logUiEvent("動きを抑えるを切り替えた", { 抑える: String(reduce) }).catch(() => undefined);
    },
  };
});

export function useReduceMotion(): boolean {
  return useMotionStore((s) => s.configured ?? s.osReduce);
}
