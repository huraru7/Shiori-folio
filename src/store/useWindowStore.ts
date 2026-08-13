import { create } from "zustand";

// デスクトップ型フリースペース(2026-08-12、UI/UX改善指示書4・5章)のウィンドウ状態。
// 位置・サイズ・重なり順(zIndex)・フルスクリーン状態をここで一元管理し、
// OsWindow(見た目・ドラッグ/リサイズ操作)とDock(開閉トグル・ドット表示)の
// 両方から参照できるようにする。永続化はせず、セッション内(メモリ)のみで
// 保持する(再起動でリセットされる)。
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface DesktopWindowState extends Rect {
  title: string;
  // 画面に表示されているか。閉じる/最小化のどちらでもfalseになる。
  visible: boolean;
  // Dockにドットを出すかどうか。「開いたことがあり、まだ閉じていない」を表す
  // (最小化中でもtrueのまま。閉じるとfalseになる)。
  open: boolean;
  zIndex: number;
  fullscreen: boolean;
  // フルスクリーン化する直前の位置・サイズ(復元用)。
  prevRect: Rect | null;
}

interface WindowStore {
  windows: Record<string, DesktopWindowState>;
  nextZIndex: number;
  registerWindow: (id: string, defaults: { title: string } & Rect) => void;
  openWindow: (id: string) => void;
  closeWindow: (id: string) => void;
  minimizeWindow: (id: string) => void;
  // Dockアイコンのクリック用。表示中なら最小化、非表示なら開く(+最前面化)。
  toggleWindow: (id: string) => void;
  focusWindow: (id: string) => void;
  moveWindow: (id: string, x: number, y: number) => void;
  resizeWindow: (id: string, width: number, height: number) => void;
  // フリースペースいっぱいに広げる/直前の位置・サイズに復元する。
  toggleFullscreen: (id: string, areaWidth: number, areaHeight: number) => void;
  // フリースペース自体のリサイズに追従させる(4章)。フルスクリーン中は
  // エリアに合わせて追従、通常時ははみ出す分だけ縮小・位置を引き戻す。
  clampToArea: (id: string, areaWidth: number, areaHeight: number) => void;
}

export const useWindowStore = create<WindowStore>((set, get) => ({
  windows: {},
  nextZIndex: 10,

  registerWindow: (id, defaults) => {
    if (get().windows[id]) return;
    const zIndex = get().nextZIndex;
    set((s) => ({
      windows: {
        ...s.windows,
        [id]: { ...defaults, visible: false, open: false, zIndex, fullscreen: false, prevRect: null },
      },
      nextZIndex: zIndex + 1,
    }));
  },

  openWindow: (id) => {
    const zIndex = get().nextZIndex;
    set((s) => {
      const win = s.windows[id];
      if (!win) return s;
      return {
        windows: { ...s.windows, [id]: { ...win, visible: true, open: true, zIndex } },
        nextZIndex: zIndex + 1,
      };
    });
  },

  closeWindow: (id) => {
    set((s) => {
      const win = s.windows[id];
      if (!win) return s;
      return { windows: { ...s.windows, [id]: { ...win, visible: false, open: false } } };
    });
  },

  minimizeWindow: (id) => {
    set((s) => {
      const win = s.windows[id];
      if (!win) return s;
      return { windows: { ...s.windows, [id]: { ...win, visible: false } } };
    });
  },

  toggleWindow: (id) => {
    const win = get().windows[id];
    if (!win) return;
    if (win.visible) {
      get().minimizeWindow(id);
    } else {
      get().openWindow(id);
    }
  },

  focusWindow: (id) => {
    const zIndex = get().nextZIndex;
    set((s) => {
      const win = s.windows[id];
      if (!win) return s;
      return {
        windows: { ...s.windows, [id]: { ...win, zIndex } },
        nextZIndex: zIndex + 1,
      };
    });
  },

  moveWindow: (id, x, y) => {
    set((s) => {
      const win = s.windows[id];
      if (!win || win.fullscreen) return s;
      return { windows: { ...s.windows, [id]: { ...win, x, y } } };
    });
  },

  resizeWindow: (id, width, height) => {
    set((s) => {
      const win = s.windows[id];
      if (!win || win.fullscreen) return s;
      return { windows: { ...s.windows, [id]: { ...win, width, height } } };
    });
  },

  toggleFullscreen: (id, areaWidth, areaHeight) => {
    const zIndex = get().nextZIndex;
    set((s) => {
      const win = s.windows[id];
      if (!win) return s;
      if (win.fullscreen) {
        const restore = win.prevRect ?? { x: win.x, y: win.y, width: win.width, height: win.height };
        return {
          windows: {
            ...s.windows,
            [id]: { ...win, ...restore, fullscreen: false, prevRect: null, zIndex },
          },
          nextZIndex: zIndex + 1,
        };
      }
      const prevRect: Rect = { x: win.x, y: win.y, width: win.width, height: win.height };
      return {
        windows: {
          ...s.windows,
          [id]: { ...win, x: 0, y: 0, width: areaWidth, height: areaHeight, fullscreen: true, prevRect, zIndex },
        },
        nextZIndex: zIndex + 1,
      };
    });
  },

  clampToArea: (id, areaWidth, areaHeight) => {
    set((s) => {
      const win = s.windows[id];
      if (!win) return s;
      if (win.fullscreen) {
        return { windows: { ...s.windows, [id]: { ...win, x: 0, y: 0, width: areaWidth, height: areaHeight } } };
      }
      // ウィンドウ自体がフリースペースより大きい場合、最小サイズより
      // 画面内に収まることを優先して縮める(4章)。
      const width = Math.min(win.width, Math.max(0, areaWidth));
      const height = Math.min(win.height, Math.max(0, areaHeight));
      const x = Math.max(0, Math.min(win.x, areaWidth - width));
      const y = Math.max(0, Math.min(win.y, areaHeight - height));
      if (width === win.width && height === win.height && x === win.x && y === win.y) return s;
      return { windows: { ...s.windows, [id]: { ...win, x, y, width, height } } };
    });
  },
}));
