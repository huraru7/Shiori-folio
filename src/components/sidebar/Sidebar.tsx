import { useEffect, useState, type CSSProperties } from "react";
import { useMainViewStore } from "../../store/useMainViewStore";
import { useSidebarItems } from "../../store/useSidebarStore";
import type { SidebarItem } from "../../lib/sidebarItems";
import "./Sidebar.css";

// アイコンのドット絵の大きさ(ドット)
const ICON_DOTS = 24;

// ウィンドウの高さに合わせたアイコンの大きさ(CSSのpx)。ドット絵をにじませないよう、画面の実際の
// 画素で24ドットの整数倍になる大きさに寄せる(拡大率が125%などのときも崩れないように)。
function iconSize(): number {
  const dpr = window.devicePixelRatio || 1;
  const wanted = Math.min(48, Math.max(24, window.innerHeight / 22));
  const scale = Math.max(1, Math.round((wanted * dpr) / ICON_DOTS));
  return (scale * ICON_DOTS) / dpr;
}

function useIconSize(): number {
  const [size, setSize] = useState(iconSize);
  useEffect(() => {
    const update = () => setSize(iconSize());
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);
  return size;
}

interface Tip {
  label: string;
  top: number;
  left: number;
}

// 左の縦の列(詩織Ver6.0の段階2)。上に画面、線の下に小道具、いちばん下に拡張機能と設定。
// アイコンだけを並べ、名前は指を乗せたときに、その項目の右に出す(出す位置は、指を乗せた項目の
// いまの位置から決める。ウィンドウの大きさを変えてもずれないように)。
// 並べ替えと非表示は、設定の画面の「列と演出」で行う。
function Sidebar() {
  const { visible } = useSidebarItems();
  const size = useIconSize();
  const [tip, setTip] = useState<Tip | null>(null);
  const screens = visible.filter((i) => i.group === "screen");
  const tools = visible.filter((i) => i.group === "tool");
  const bottom = visible.filter((i) => i.group === "bottom");

  const showTip = (label: string, el: HTMLElement) => {
    const rect = el.getBoundingClientRect();
    setTip({ label, top: rect.top + rect.height / 2, left: rect.right + 8 });
  };
  const hideTip = () => setTip(null);

  // 大きさの変化やスクロールで項目が動いたら、名前は消す(古い位置に残さない)
  useEffect(() => {
    window.addEventListener("resize", hideTip);
    return () => window.removeEventListener("resize", hideTip);
  }, []);

  const style = { "--sidebar-icon": `${size}px` } as CSSProperties;
  const button = (item: SidebarItem) => (
    <SidebarButton key={item.id} item={item} onEnter={showTip} onLeave={hideTip} />
  );

  return (
    <nav className="sidebar" aria-label="画面と小道具" style={style} onScroll={hideTip}>
      <div className="sidebar__group">{screens.map(button)}</div>
      {tools.length > 0 && (
        <>
          <hr className="sidebar__line" />
          <div className="sidebar__group">{tools.map(button)}</div>
        </>
      )}
      <div className="sidebar__group sidebar__group--bottom">{bottom.map(button)}</div>
      {tip && (
        <span className="sidebar__tip" role="tooltip" style={{ top: tip.top, left: tip.left }}>
          {tip.label}
        </span>
      )}
    </nav>
  );
}

function SidebarButton({
  item,
  onEnter,
  onLeave,
}: {
  item: SidebarItem;
  onEnter: (label: string, el: HTMLElement) => void;
  onLeave: () => void;
}) {
  const active = useMainViewStore((s) => (item.group === "tool" ? s.openTool === item.ext?.name : s.active === item.id));
  const setActive = useMainViewStore((s) => s.setActive);
  const toggleTool = useMainViewStore((s) => s.toggleTool);

  const onClick = () => {
    onLeave();
    if (item.group === "tool" && item.ext) toggleTool(item.ext.name);
    else setActive(item.id);
  };

  return (
    <button
      type="button"
      className={`sidebar__item${active ? " sidebar__item--active" : ""}`}
      data-sidebar-id={item.id}
      data-ripple
      aria-pressed={active}
      aria-label={item.label}
      onClick={onClick}
      onMouseEnter={(e) => onEnter(item.label, e.currentTarget)}
      onMouseLeave={onLeave}
      onFocus={(e) => onEnter(item.label, e.currentTarget)}
      onBlur={onLeave}
    >
      {item.icon ? (
        <img className="sidebar__icon" src={item.icon} alt="" draggable={false} />
      ) : (
        <span className="sidebar__letter">{item.label.slice(0, 1)}</span>
      )}
    </button>
  );
}

export default Sidebar;
