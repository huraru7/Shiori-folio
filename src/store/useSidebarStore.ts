import { create } from "zustand";
import { useMemo } from "react";
import { api } from "../api/tauri";
import type { SidebarLayout } from "../types";
import { allItems, arrange, visibleItems, type SidebarItem } from "../lib/sidebarItems";
import { useExtensionsStore } from "./useExtensionsStore";

// 左の縦の列の並びと非表示(詩織Ver6.0の段階2)。data/ui/sidebar.jsonに保存し、次の起動でも保つ。
interface SidebarStore {
  layout: SidebarLayout;
  // data/ui/icons/に差し替えのPNGがある、本体の画面のid
  iconOverrides: string[];
  loaded: boolean;
  error: string | null;
  load: () => Promise<void>;
  // 同じ仲間(画面・小道具)の中で、1つ上か下へ動かす。itemsは、いまの並びの全部の項目
  move: (items: SidebarItem[], id: string, delta: -1 | 1) => Promise<void>;
  setHidden: (id: string, hidden: boolean) => Promise<void>;
}

export const useSidebarStore = create<SidebarStore>((set, get) => {
  // 保存は押した順に1つずつ行う(続けて押したときに、古い並びが後から書かれないように)
  let queue: Promise<void> = Promise.resolve();
  const save = (layout: SidebarLayout) => {
    set({ layout });
    queue = queue.then(async () => {
      try {
        await api.setSidebarLayout(layout);
        set({ error: null });
      } catch (err) {
        set({ error: String(err) });
      }
    });
    return queue;
  };

  return {
    layout: { order: [], hidden: [] },
    iconOverrides: [],
    loaded: false,
    error: null,
    load: async () => {
      const [layout, iconOverrides] = await Promise.all([
        api.getSidebarLayout().catch(() => ({ order: [], hidden: [] })),
        api.listIconOverrides().catch(() => []),
      ]);
      set({ layout, iconOverrides, loaded: true });
    },
    move: async (items, id, delta) => {
      const index = items.findIndex((i) => i.id === id);
      const item = items[index];
      if (!item) return;
      // 同じ仲間の隣と入れ替える(画面を小道具の下へは動かさない)
      let j = index + delta;
      while (j >= 0 && j < items.length && items[j].group !== item.group) j += delta;
      if (j < 0 || j >= items.length) return;
      const order = items.map((i) => i.id as string);
      [order[index], order[j]] = [order[j], order[index]];
      await save({ ...get().layout, order });
    },
    setHidden: async (id, hidden) => {
      const current = get().layout.hidden.filter((h) => h !== id);
      await save({ ...get().layout, hidden: hidden ? [...current, id] : current });
    },
  };
});

// 記録の並びに合わせた、全部の項目(設定の画面の一覧用)と、列に出す項目。
export function useSidebarItems(): { all: SidebarItem[]; visible: SidebarItem[] } {
  const layout = useSidebarStore((s) => s.layout);
  const iconOverrides = useSidebarStore((s) => s.iconOverrides);
  const extensions = useExtensionsStore((s) => s.list);
  return useMemo(() => {
    const all = arrange(allItems(extensions ?? [], iconOverrides), layout);
    return { all, visible: visibleItems(all, layout) };
  }, [layout, iconOverrides, extensions]);
}
