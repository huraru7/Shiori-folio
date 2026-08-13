import { useEffect, useRef, type ReactNode } from "react";
import { useWindowStore } from "../../store/useWindowStore";
import "./OsWindow.css";

interface Props {
  id: string;
  title: string;
  defaultX: number;
  defaultY: number;
  defaultWidth: number;
  defaultHeight: number;
  minWidth?: number;
  minHeight?: number;
  containerRef: React.RefObject<HTMLDivElement | null>;
  children: ReactNode;
  // 図書館・設定のように、中身が検索バー固定+一覧スクロールなど自前で
  // レイアウト・スクロールを管理するウィンドウ向け。trueだとos-window__body
  // の余白を消し、高さいっぱいのflex列コンテナにする。
  fillBody?: boolean;
}

// デスクトップ型フリースペースの汎用ウィンドウ(2026-08-12、UI/UX改善指示書
// 「デスクトップ型ウィンドウシステムの本実装」)。ドラッグ移動・リサイズ(右下角)・
// フォーカス時最前面化・最小化・フルスクリーン・閉じるを担う。位置/サイズ/
// 表示状態はuseWindowStoreで一元管理し、常にマウントしたままdisplay:noneで
// 出し分ける(最小化・復元をまたいで中身のstateを保持するため)。
function OsWindow({
  id,
  title,
  defaultX,
  defaultY,
  defaultWidth,
  defaultHeight,
  minWidth = 220,
  minHeight = 160,
  containerRef,
  children,
  fillBody = false,
}: Props) {
  const win = useWindowStore((s) => s.windows[id]);
  const registerWindow = useWindowStore((s) => s.registerWindow);
  const focusWindow = useWindowStore((s) => s.focusWindow);
  const minimizeWindow = useWindowStore((s) => s.minimizeWindow);
  const closeWindow = useWindowStore((s) => s.closeWindow);
  const moveWindow = useWindowStore((s) => s.moveWindow);
  const resizeWindow = useWindowStore((s) => s.resizeWindow);
  const toggleFullscreen = useWindowStore((s) => s.toggleFullscreen);
  const clampToArea = useWindowStore((s) => s.clampToArea);

  useEffect(() => {
    registerWindow(id, { title, x: defaultX, y: defaultY, width: defaultWidth, height: defaultHeight });
    // 初回マウント時のみ登録する(以降はstore側の位置・サイズが正)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  // フリースペース自体のリサイズに追従させる(4章)。表示されるたび
  // (タスクバーからの復元含む)と、フリースペースのサイズが変わるたびに
  // クランプし直す。
  useEffect(() => {
    if (!containerRef.current) return;
    const clamp = () => {
      const rect = containerRef.current?.getBoundingClientRect();
      if (rect) clampToArea(id, rect.width, rect.height);
    };
    if (win?.visible) clamp();
    const observer = new ResizeObserver(clamp);
    observer.observe(containerRef.current);
    return () => observer.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, win?.visible, containerRef]);

  const dragState = useRef<{ offX: number; offY: number } | null>(null);
  const resizeState = useRef<{ startW: number; startH: number; startX: number; startY: number } | null>(null);

  const handleTitlebarMouseDown = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest(".os-window__btn")) return;
    if (!win || win.fullscreen) return;
    focusWindow(id);
    const rect = (e.currentTarget as HTMLElement).closest(".os-window")!.getBoundingClientRect();
    dragState.current = { offX: e.clientX - rect.left, offY: e.clientY - rect.top };

    const handleMove = (ev: MouseEvent) => {
      if (!dragState.current || !containerRef.current) return;
      const areaRect = containerRef.current.getBoundingClientRect();
      let nx = ev.clientX - areaRect.left - dragState.current.offX;
      let ny = ev.clientY - areaRect.top - dragState.current.offY;
      nx = Math.max(0, Math.min(nx, areaRect.width - win.width));
      ny = Math.max(0, Math.min(ny, areaRect.height - win.height));
      moveWindow(id, nx, ny);
    };
    const handleUp = () => {
      dragState.current = null;
      document.removeEventListener("mousemove", handleMove);
      document.removeEventListener("mouseup", handleUp);
    };
    document.addEventListener("mousemove", handleMove);
    document.addEventListener("mouseup", handleUp);
  };

  const handleResizeMouseDown = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!win || win.fullscreen) return;
    focusWindow(id);
    resizeState.current = { startW: win.width, startH: win.height, startX: e.clientX, startY: e.clientY };

    const handleMove = (ev: MouseEvent) => {
      if (!resizeState.current || !containerRef.current) return;
      const areaRect = containerRef.current.getBoundingClientRect();
      const dx = ev.clientX - resizeState.current.startX;
      const dy = ev.clientY - resizeState.current.startY;
      const maxW = areaRect.width - win.x;
      const maxH = areaRect.height - win.y;
      const nw = Math.max(minWidth, Math.min(resizeState.current.startW + dx, maxW));
      const nh = Math.max(minHeight, Math.min(resizeState.current.startH + dy, maxH));
      resizeWindow(id, nw, nh);
    };
    const handleUp = () => {
      resizeState.current = null;
      document.removeEventListener("mousemove", handleMove);
      document.removeEventListener("mouseup", handleUp);
    };
    document.addEventListener("mousemove", handleMove);
    document.addEventListener("mouseup", handleUp);
  };

  const handleToggleFullscreen = () => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    toggleFullscreen(id, rect.width, rect.height);
  };

  if (!win) return null;

  return (
    <div
      className="os-window"
      style={{
        left: win.x,
        top: win.y,
        width: win.width,
        height: win.height,
        zIndex: win.zIndex,
        display: win.visible ? "flex" : "none",
      }}
      onMouseDown={() => focusWindow(id)}
    >
      <div className="os-window__titlebar" onMouseDown={handleTitlebarMouseDown}>
        <span className="os-window__title">{title}</span>
        <div className="os-window__controls">
          <button className="os-window__btn" onClick={() => minimizeWindow(id)} aria-label="最小化" title="最小化">
            −
          </button>
          <button
            className="os-window__btn"
            onClick={handleToggleFullscreen}
            aria-label={win.fullscreen ? "元のサイズに戻す" : "フルスクリーン"}
            title={win.fullscreen ? "元のサイズに戻す" : "フルスクリーン"}
          >
            {win.fullscreen ? "⧉" : "⛶"}
          </button>
          <button className="os-window__btn" onClick={() => closeWindow(id)} aria-label="閉じる" title="閉じる">
            ×
          </button>
        </div>
      </div>
      <div className={`os-window__body${fillBody ? " os-window__body--fill" : ""}`}>{children}</div>
      {!win.fullscreen && <div className="os-window__resize-handle" onMouseDown={handleResizeMouseDown} />}
    </div>
  );
}

export default OsWindow;
