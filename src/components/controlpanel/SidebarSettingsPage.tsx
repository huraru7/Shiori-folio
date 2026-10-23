import { useSidebarItems, useSidebarStore } from "../../store/useSidebarStore";
import { useMotionStore, useReduceMotion } from "../../store/useMotionStore";
import { isHideable, type SidebarItem } from "../../lib/sidebarItems";
import { ToggleRow } from "./shared";

const GROUP_LABEL: Record<SidebarItem["group"], string> = {
  screen: "画面",
  tool: "小道具",
  bottom: "いちばん下",
};

// 左の縦の列の並べ替えと非表示、演出の設定(詩織Ver6.0の段階2)。
// 変えるとすぐにdata/ui/sidebar.json(演出はconfig.json)に保存し、次の起動でも保つ。
// 画面と小道具は、それぞれの仲間の中で並べ替える。拡張機能と設定は、隠すと戻れなくなるので隠せない。
export function SidebarSettingsPage() {
  const { all } = useSidebarItems();
  const hidden = useSidebarStore((s) => s.layout.hidden);
  const error = useSidebarStore((s) => s.error);
  const move = useSidebarStore((s) => s.move);
  const setHidden = useSidebarStore((s) => s.setHidden);
  const reduce = useReduceMotion();
  const setReduce = useMotionStore((s) => s.set);
  const configured = useMotionStore((s) => s.configured);

  const groups = (["screen", "tool", "bottom"] as const).map((g) => ({
    group: g,
    items: all.filter((i) => i.group === g),
  }));

  return (
    <>
      <div className="control-panel__page-head">
        <h1>列と演出</h1>
        <p>左の縦の列の並びと、見せる項目を選びます。変えるとすぐに保存され、次の起動でも保たれます。</p>
      </div>
      {error && <p className="control-panel__error">{error}</p>}
      {groups.map(({ group, items }) =>
        items.length === 0 ? null : (
          <div key={group} className="control-panel__panel" data-sidebar-settings-group={group}>
            <div className="control-panel__panel-label">{GROUP_LABEL[group]}</div>
            <ul className="sidebar-settings__list">
              {items.map((item, index) => (
                <li key={item.id} className="sidebar-settings__row" data-sidebar-settings-id={item.id}>
                  {item.icon ? (
                    <img className="sidebar-settings__icon" src={item.icon} alt="" />
                  ) : (
                    <span className="sidebar-settings__letter">{item.label.slice(0, 1)}</span>
                  )}
                  <span className="sidebar-settings__label">{item.label}</span>
                  {group !== "bottom" && (
                    <>
                      <button
                        type="button"
                        className="control-panel__btn"
                        disabled={index === 0}
                        onClick={() => move(all, item.id, -1)}
                        aria-label={`${item.label}を上へ`}
                      >
                        上へ
                      </button>
                      <button
                        type="button"
                        className="control-panel__btn"
                        disabled={index === items.length - 1}
                        onClick={() => move(all, item.id, 1)}
                        aria-label={`${item.label}を下へ`}
                      >
                        下へ
                      </button>
                    </>
                  )}
                  <label className="sidebar-settings__show">
                    <input
                      type="checkbox"
                      checked={!isHideable(item) || !hidden.includes(item.id)}
                      disabled={!isHideable(item)}
                      onChange={(e) => setHidden(item.id, !e.target.checked)}
                    />
                    見せる
                  </label>
                </li>
              ))}
            </ul>
          </div>
        ),
      )}
      <div className="control-panel__panel">
        <div className="control-panel__panel-label">演出</div>
        <ToggleRow label="動きを抑える(水紋と星を出さない)" checked={reduce} onChange={(v) => setReduce(v)} />
        {configured === null && (
          <p className="sidebar-settings__note">いまは、OSの「視差効果を減らす」の設定に従っています。</p>
        )}
      </div>
    </>
  );
}
