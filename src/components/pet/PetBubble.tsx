import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../../api/tauri";
import type { Notice } from "../../types";
import "./pet.css";
import "./PetBubble.css";

// 知らせの吹き出し(詩織Ver6.0の段階4)。ペットの形のときに、本体がペットの頭の上に出す。
// 段階3の札と同じく、タイマーは押すまで残り、ほかは10秒で消える。音はウィンドウの側(隠れていても
// 動いている)で鳴らすので、ここでは鳴らさない。いくつも重なったら、新しいものを出し、残りは件数で示す。
// 押すと、その知らせの元へ移る(本体の pet_open_source)。全部消えたら、本体に吹き出しを隠してもらう。

const FADE_MS = 10_000;

function PetBubble() {
  const [notices, setNotices] = useState<Notice[]>([]);
  const petMode = useRef(false);

  useEffect(() => {
    api
      .petStatus()
      .then((s) => {
        petMode.current = s.mode === "pet";
      })
      .catch(() => undefined);
    const remove = (id: number) => setNotices((ns) => ns.filter((n) => n.id !== id));
    const stops: Promise<() => void>[] = [
      listen<string>("pet://mode", (e) => {
        petMode.current = e.payload === "pet";
        if (!petMode.current) setNotices([]);
      }),
      listen<Notice>("shiori://notify", (e) => {
        // ウィンドウの形のときの知らせは、札で出しているので受けない
        if (!petMode.current) return;
        const n = e.payload;
        setNotices((ns) => [n, ...ns]);
        if (n.kind !== "timer") setTimeout(() => remove(n.id), FADE_MS);
      }),
    ];
    return () => {
      stops.forEach((p) => p.then((u) => u()));
    };
  }, []);

  useEffect(() => {
    if (notices.length === 0) api.petBubbleDone().catch(() => undefined);
  }, [notices.length]);

  const n = notices[0];
  if (!n) return null;
  const dismiss = () => setNotices((ns) => ns.filter((x) => x.id !== n.id));
  return (
    <div className="pet-bubble" data-notice-kind={n.kind}>
      <button
        type="button"
        className="pet-bubble__main"
        onClick={() => {
          api.petOpenSource(n).catch(() => undefined);
          dismiss();
        }}
      >
        <span className="pet-bubble__title">{n.title}</span>
        {n.body && <span className="pet-bubble__body">{n.body}</span>}
        {notices.length > 1 && <span className="pet-bubble__more">ほかに{notices.length - 1}件</span>}
      </button>
      <button type="button" className="pet-bubble__close" onClick={dismiss} aria-label="知らせを閉じる">
        ×
      </button>
    </div>
  );
}

export default PetBubble;
