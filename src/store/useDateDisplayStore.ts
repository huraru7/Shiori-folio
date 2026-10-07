import { create } from "zustand";
import { api } from "../api/tauri";

export interface DateDisplayConfig {
  showYear: boolean;
  showMonth: boolean;
  showDay: boolean;
  showWeekday: boolean;
  showSeconds: boolean;
}

interface DateDisplayStore {
  config: DateDisplayConfig;
  // config.jsonから読み直す。失敗したら今の値のまま(ヘッダー自体は表示したいため)。
  load: () => void;
  // 設定画面で保存した直後に、ヘッダーへすぐ反映するために使う。
  set: (config: DateDisplayConfig) => void;
}

// ヘッダーの日付・時刻の表示設定(詩織Ver3.9)。以前はヘッダーが起動時に1回だけ
// 読んでいたため、設定画面で保存しても再起動するまで反映されなかった。
export const useDateDisplayStore = create<DateDisplayStore>((set) => ({
  config: { showYear: true, showMonth: true, showDay: true, showWeekday: false, showSeconds: false },
  load: () => {
    api
      .getConfig()
      .then((c) =>
        set({
          config: {
            showYear: c.showYear,
            showMonth: c.showMonth,
            showDay: c.showDay,
            showWeekday: c.showWeekday,
            showSeconds: c.showSeconds,
          },
        }),
      )
      .catch(() => undefined);
  },
  set: (config) => set({ config }),
}));
