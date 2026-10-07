import { create } from "zustand";
import { api } from "../api/tauri";
import type { AppMode } from "../types";
import { useShioriStore } from "./useShioriStore";

interface AppModeStore {
  // config.jsonから読み込むまではnull(画面はどちらのモードとも決めつけない)。
  mode: AppMode | null;
  // 切り替えの最中(会話用LLMの起動には数十秒かかる)。
  switching: boolean;
  error: string | null;
  load: () => void;
  change: (mode: AppMode) => Promise<void>;
}

// 会話モード/外部AIモード(詩織Ver4.0)。外部AIモードでは会話用LLMを止めて、Claudeなど
// 外部のAIと使う。モードはconfig.jsonに保存され、再起動しても保たれる。
export const useAppModeStore = create<AppModeStore>((set, get) => ({
  mode: null,
  switching: false,
  error: null,
  load: () => {
    api
      .getConfig()
      .then((c) => set({ mode: c.mode }))
      .catch((e) => set({ mode: "conversation", error: String(e) }));
  },
  change: async (mode) => {
    if (get().switching || get().mode === mode) return;
    set({ switching: true, error: null });
    try {
      // 切り替えた時点で会話・音声を止めるため、モードは先に変える。
      set({ mode });
      const llm = await api.setAppMode(mode);
      useShioriStore.setState((s) => ({ serviceStatuses: { ...s.serviceStatuses, llm } }));
      if (mode === "conversation" && !llm.healthy) {
        set({ error: llm.error ?? "会話用のAIを起動できませんでした" });
      }
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ switching: false });
    }
  },
}));
