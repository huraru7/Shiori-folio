import { create } from "zustand";
import { api } from "../api/tauri";
import type { PendingItems } from "../types";

// 要確認の記録(詩織Ver6.0の段階2で、上のタブの右から上の帯に移した)。
// 上の帯の件数の印と、ホームの要確認の欄が同じものを見る。
interface PendingStore {
  items: PendingItems | null;
  open: boolean;
  load: () => Promise<void>;
  setItems: (items: PendingItems) => void;
  setOpen: (open: boolean) => void;
}

export const usePendingStore = create<PendingStore>((set) => ({
  items: null,
  open: false,
  load: async () => {
    try {
      set({ items: await api.listPendingItems() });
    } catch {
      // 取得に失敗しても、ボタン自体は使えるように件数の印を出さないだけにする。
    }
  },
  setItems: (items) => set({ items }),
  setOpen: (open) => set({ open }),
}));

export function pendingBadgeCount(items: PendingItems): number {
  // 件数の印は「未着手」のみを数える(保留中はモーダルの別タブで確認する対象で、
  // 対応不要なものとして未着手カウントから除外する、詩織Ver2.0 Phase 8の方針)。
  return (
    items.tags.filter((t) => t.status === "pending").length +
    items.projects.filter((p) => p.status === "pending").length +
    items.inbox.filter((i) => i.reviewStatus === null).length
  );
}
