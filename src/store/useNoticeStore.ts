import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api/tauri";
import type { Notice, NoticeKind, NotifySound } from "../types";
import { playChime } from "../lib/chime";
import { usePendingStore } from "./usePendingStore";

// 知らせの札(詩織Ver6.0の段階3)。本体の知らせの入口(notify.rs)が送る `shiori://notify` を受け、
// 画面の右上に札を積む。音は、種類ごとにconfig.jsonのnotify.soundで鳴らすかを決める。
// タスクバーの点滅は本体の側で行う。

const EVENT = "shiori://notify";
// 同時に出す札の数
const MAX_SHOWN = 5;
// タイマー以外の札が消えるまでの時間
const FADE_MS = 10_000;

const DEFAULT_SOUND: NotifySound = { timer: true, claude: false, pending: false, extension: false };

interface NoticeStore {
  notices: Notice[];
  sound: NotifySound;
  load: () => Promise<void>;
  // 本体の知らせを受け始める。止める関数を返す
  subscribe: () => Promise<() => void>;
  dismiss: (id: number) => void;
  setSound: (kind: NoticeKind, on: boolean) => Promise<void>;
}

export const useNoticeStore = create<NoticeStore>((set, get) => {
  const receive = (notice: Notice) => {
    // 新しいものを上に。溢れた分は古いものから消す
    set({ notices: [notice, ...get().notices].slice(0, MAX_SHOWN) });
    if (get().sound[notice.kind]) playChime(notice.kind);
    if (notice.kind !== "timer") {
      setTimeout(() => get().dismiss(notice.id), FADE_MS);
    }
    // 要確認が増えたら、上の帯の件数の印も読み直す
    if (notice.kind === "pending") usePendingStore.getState().load();
  };

  return {
    notices: [],
    sound: DEFAULT_SOUND,
    load: async () => {
      try {
        const config = await api.getConfig();
        set({ sound: { ...DEFAULT_SOUND, ...config.notifySound } });
      } catch {
        // 読めなければ既定のまま
      }
    },
    subscribe: async () => listen<Notice>(EVENT, (e) => receive(e.payload)),
    dismiss: (id) => set({ notices: get().notices.filter((n) => n.id !== id) }),
    setSound: async (kind, on) => {
      const sound = { ...get().sound, [kind]: on };
      set({ sound });
      await api.setConfig({ notifySound: sound });
      api.logUiEvent("知らせの音を切り替えた", { 種類: kind, 鳴らす: String(on) }).catch(() => undefined);
    },
  };
});
