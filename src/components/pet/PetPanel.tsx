import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../../api/tauri";
import { useExtensionsStore } from "../../store/useExtensionsStore";
import { useSidebarStore } from "../../store/useSidebarStore";
import { askExtension } from "../../lib/selfTest";
import ExtensionFrame from "../extensions/ExtensionFrame";
import "./pet.css";
import "./PetPanel.css";

// 本のメニューと、ペットの横の小道具の枠(詩織Ver6.0の段階4)。ペットを押すと、本体がこの窓を
// ペットの横に出す。いちばん上は「ウィンドウに戻る」、そのあとに説明書のplacesにpetを含む小道具を、
// 列の並びの順に並べる。小道具は本の代わりに枠で開き、閉じても中身(iframe)は残す。
// 本の形は、見本(prototypes/ver6-s4/book-menu/)から選んだ案A「横に見開きで開く」(ふらる、10/23)。
// 手の本がふくらみ、遠い方のページがめくれて見開きになる。ペットの側のページに項目、もう片方は見出しと続きの余白。

interface PetTool {
  name: string;
  label: string;
  entry: string;
}

function PetPanel() {
  const extensions = useExtensionsStore((s) => s.list);
  const layout = useSidebarStore((s) => s.layout);
  const [open, setOpen] = useState<string | null>(null);
  // 本がペットのどちら側に開いたか(本体が知らせる)と、開く動きのための印
  const [side, setSide] = useState<"left" | "right">("right");
  const [spread, setSpread] = useState(false);
  // 本と小道具の枠の大きさ(設定の割合。1が100%)
  const [scale, setScale] = useState(1);
  const [opened, setOpened] = useState<string[]>([]);
  const slots = useRef(new Map<string, HTMLDivElement>());

  useEffect(() => {
    useExtensionsStore.getState().load();
    useSidebarStore.getState().load();
  }, []);

  const tools: PetTool[] = useMemo(() => {
    const list = (extensions ?? []).filter(
      (e) => e.status === "ready" && e.kind === "ui" && e.name && e.entry && e.places.includes("pet"),
    );
    const rank = (name: string) => {
      const i = layout.order.indexOf(`ext:${name}`);
      return i >= 0 ? i : layout.order.length;
    };
    return list
      .map((e) => ({ name: e.name as string, label: e.title ?? (e.name as string), entry: e.entry as string }))
      .sort((a, b) => rank(a.name) - rank(b.name));
  }, [extensions, layout]);

  // 並べた項目を本体に知らせる(自己確認で使う)
  useEffect(() => {
    if (extensions) api.petPanelReport({ items: ["window", ...tools.map((t) => t.name)] }).catch(() => undefined);
  }, [extensions, tools]);

  const showTool = (name: string) => {
    setOpen(name);
    setOpened((o) => (o.includes(name) ? o : [...o, name]));
  };

  useEffect(() => {
    const stops: Promise<() => void>[] = [
      listen<string>("pet://open-tool", (e) => showTool(e.payload)),
      listen<{ side: "left" | "right"; scale: number }>("pet://panel-open", (e) => {
        setSide(e.payload.side);
        setScale(e.payload.scale);
        // 一度閉じた形で出してから開く(開く動きを見せる)
        setSpread(false);
        requestAnimationFrame(() => requestAnimationFrame(() => setSpread(true)));
      }),
      // 閉じたら本の最初のページに戻す(小道具の中身は残す)
      listen("pet://panel-closed", () => {
        setOpen(null);
        setSpread(false);
      }),
      // 自己確認: 小道具を開き、その中の確かめを走らせて、結果を本体に知らせる
      listen<string>("pet://selftest-tool", async (e) => {
        const name = e.payload;
        showTool(name);
        let iframe: HTMLIFrameElement | null = null;
        for (let i = 0; i < 50 && !iframe; i++) {
          await new Promise((r) => setTimeout(r, 100));
          iframe = slots.current.get(name)?.querySelector("iframe") ?? null;
        }
        const done = await askExtension(iframe, name);
        await api.petPanelReport({ tool: { name, ok: done?.ok ?? 0, ng: done ? done.ng : 1 } });
      }),
    ];
    return () => {
      stops.forEach((p) => p.then((u) => u()));
    };
  }, []);

  const current = tools.find((t) => t.name === open);

  return (
    <div className={`pet-panel pet-panel--${side}`} style={{ zoom: scale }}>
      <div className={`pet-panel__book pet-panel__book--${side}${spread ? " is-open" : ""}`} hidden={!!current}>
        <div className="pet-panel__spread">
          <div className="pet-panel__page pet-panel__page--near">
            <p className="pet-panel__page-title">しおりの本</p>
            <ul className="pet-panel__items">
              <li>
                <button type="button" className="pet-panel__item pet-panel__item--back" onClick={() => api.petLeave()}>
                  <span className="pet-panel__seal">窓</span>ウィンドウに戻る
                </button>
              </li>
              {tools.map((t) => (
                <li key={t.name}>
                  <button type="button" className="pet-panel__item" data-pet-tool={t.name} onClick={() => showTool(t.name)}>
                    <span className="pet-panel__seal">{t.label.slice(0, 1)}</span>
                    {t.label}
                  </button>
                </li>
              ))}
            </ul>
            {extensions && tools.length === 0 && <p className="pet-panel__empty">ペットで使える小道具はありません</p>}
            <div className="pet-panel__page-foot">一</div>
          </div>
          <div className="pet-panel__page pet-panel__page--far" aria-hidden="true">
            <p className="pet-panel__page-title">つづき</p>
            <div className="pet-panel__ruled" />
            <div className="pet-panel__page-foot">二</div>
          </div>
        </div>
      </div>
      <div className="pet-panel__tool" hidden={!current}>
        <div className="pet-panel__bar">
          <button type="button" className="pet-panel__back" onClick={() => setOpen(null)}>
            本に戻る
          </button>
          <span className="pet-panel__title">{current?.label}</span>
          <button type="button" className="pet-panel__back" onClick={() => api.petClosePanel()} aria-label="閉じる">
            ×
          </button>
        </div>
        {tools
          .filter((t) => opened.includes(t.name))
          .map((t) => (
            <div
              key={t.name}
              className="pet-panel__slot"
              hidden={t.name !== open}
              ref={(el) => {
                if (el) slots.current.set(t.name, el);
                else slots.current.delete(t.name);
              }}
            >
              <ExtensionFrame name={t.name} entry={t.entry} title={t.label} />
            </div>
          ))}
      </div>
    </div>
  );
}

export default PetPanel;
