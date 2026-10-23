import { useEffect } from "react";
import { useNoticeStore } from "../../store/useNoticeStore";
import type { NoticeKind } from "../../types";
import { ToggleRow } from "./shared";

const KINDS: { kind: NoticeKind; label: string }[] = [
  { kind: "timer", label: "タイマーの終わり" },
  { kind: "claude", label: "Claudeの作業の終わり" },
  { kind: "pending", label: "要確認が増えたとき" },
  { kind: "extension", label: "拡張機能からの知らせ" },
];

// 知らせの設定(詩織Ver6.0の段階3)。種類ごとに、音を鳴らすかを選ぶ。変えるとすぐにconfig.jsonへ保存する。
export function NotifySettingsPage() {
  const sound = useNoticeStore((s) => s.sound);
  const setSound = useNoticeStore((s) => s.setSound);

  useEffect(() => {
    useNoticeStore.getState().load();
  }, []);

  return (
    <>
      <div className="control-panel__page-head">
        <h1>知らせ</h1>
        <p>知らせは画面の右上に札で出し、詩織が前に無いときはタスクバーのアイコンが点滅します。音を鳴らす知らせを選びます。</p>
      </div>
      <div className="control-panel__panel" data-notify-settings>
        <div className="control-panel__panel-label">音を鳴らす</div>
        {KINDS.map(({ kind, label }) => (
          <ToggleRow key={kind} label={label} checked={sound[kind]} onChange={(v) => setSound(kind, v)} />
        ))}
      </div>
    </>
  );
}
