import { useEffect, useState } from "react";
import { useExtensionsStore } from "../../store/useExtensionsStore";
import { api } from "../../api/tauri";
import type { ExtensionInfo, ExtensionRunState } from "../../types";
import ExtensionFrame from "./ExtensionFrame";
import "./ExtensionsScreen.css";

const STATUS_LABEL: Record<ExtensionInfo["status"], string> = {
  ready: "使える",
  "needs-approval": "未承認",
  invalid: "読めない",
};

function runLabel(run: ExtensionRunState, restarts: number): string {
  const again = restarts > 0 ? `(再起動 ${restarts}回)` : "";
  switch (run.state) {
    case "starting":
      return `起動中${again}`;
    case "running":
      return `動いている${again}`;
    case "stopped":
      return `止まっている: ${run.reason}`;
  }
}

const KIND_LABEL: Record<"ui" | "program", string> = {
  ui: "画面の部品",
  program: "別のプログラム",
};

// 拡張機能の一覧(詩織Ver6.0の段階1の、仮の入り口)。portable/extensions/に置いた拡張機能と、
// その状態(使える・未承認・読めない)を見せ、未承認のものはここから承認できる。
// 見た目は仮で、段階2で左の縦の列に移す。
function ExtensionsScreen() {
  const list = useExtensionsStore((s) => s.list);
  const error = useExtensionsStore((s) => s.error);
  const load = useExtensionsStore((s) => s.load);
  const approve = useExtensionsStore((s) => s.approve);
  // 開いている画面の部品の拡張機能(仮。段階2で、いまの画面の横の小さな枠にする)
  const [opened, setOpened] = useState<string | null>(null);
  const openedExt = list?.find((e) => e.name === opened && e.status === "ready" && e.entry);
  // 別のプログラムの拡張機能を呼んだ結果(拡張機能の名前ごと)
  const [callResults, setCallResults] = useState<Record<string, string>>({});

  const callProvided = async (name: string, method: string) => {
    setCallResults((r) => ({ ...r, [name]: `${method} を呼んでいます...` }));
    try {
      const result = await api.callExtension(name, method, method === "greet" ? { who: "ふらる" } : {});
      setCallResults((r) => ({ ...r, [name]: `${method}: ${JSON.stringify(result)}` }));
    } catch (err) {
      const e = err as { code?: string; message?: string };
      setCallResults((r) => ({ ...r, [name]: `${method}: ${e.code ?? ""} ${e.message ?? String(err)}` }));
    }
    load();
  };

  useEffect(() => {
    if (list === null) load();
  }, [list, load]);

  return (
    <div className="extensions-screen">
      <p className="extensions-screen__lead">
        portable/extensions/ に置いた拡張機能です。詩織の起動時に読み込みます。足したり外したりしたら、詩織を起動し直してください。
        <button type="button" className="extensions-screen__close" onClick={() => load()}>
          状態を読み直す
        </button>
      </p>
      {error && <div className="extensions-screen__error">{error}</div>}
      {openedExt && openedExt.name && openedExt.entry && (
        <div className="extensions-screen__frame">
          <div className="extensions-screen__frame-bar">
            <span>{openedExt.title}</span>
            <button type="button" className="extensions-screen__close" onClick={() => setOpened(null)}>
              閉じる
            </button>
          </div>
          <ExtensionFrame
            key={openedExt.name}
            name={openedExt.name}
            entry={openedExt.entry}
            title={openedExt.title ?? openedExt.name}
          />
        </div>
      )}
      {list === null && !error && <p className="extensions-screen__empty">読み込み中...</p>}
      {list !== null && list.length === 0 && (
        <p className="extensions-screen__empty">拡張機能はまだありません。</p>
      )}
      <ul className="extensions-screen__list">
        {list?.map((ext) => (
          <li key={ext.folder} className="extensions-screen__item">
            <div className="extensions-screen__head">
              <span className="extensions-screen__title">{ext.title ?? ext.folder}</span>
              {ext.version && <span className="extensions-screen__meta">版 {ext.version}</span>}
              {ext.kind && <span className="extensions-screen__meta">{KIND_LABEL[ext.kind]}</span>}
              <span className={`extensions-screen__status extensions-screen__status--${ext.status}`}>
                {STATUS_LABEL[ext.status]}
              </span>
            </div>
            <div className="extensions-screen__folder">{ext.folder}</div>
            {ext.description && <p className="extensions-screen__desc">{ext.description}</p>}
            {ext.status === "invalid" ? (
              <p className="extensions-screen__invalid">
                {ext.errorCode} {ext.errorMessage}
              </p>
            ) : (
              <div className="extensions-screen__uses">
                使うもの: {ext.uses.length === 0 ? "なし" : ext.uses.map((u) => u.label).join("、")}
              </div>
            )}
            {ext.runtime && (
              <div className="extensions-screen__uses">動き: {runLabel(ext.runtime, ext.restarts)}</div>
            )}
            {ext.status === "ready" && ext.kind === "program" && ext.name && (
              <ProvidedButtons name={ext.name} methods={ext.provides} onCall={callProvided} />
            )}
            {ext.name && callResults[ext.name] && (
              <div className="extensions-screen__desc">{callResults[ext.name]}</div>
            )}
            {ext.status === "ready" && ext.kind === "ui" && ext.name && (
              <button
                type="button"
                className="extensions-screen__approve"
                onClick={() => setOpened(ext.name)}
              >
                開く
              </button>
            )}
            {ext.status === "needs-approval" && ext.name && (
              <button
                type="button"
                className="extensions-screen__approve"
                onClick={() => approve(ext.name as string)}
              >
                承認する
              </button>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

// 別のプログラムの拡張機能のprovidesを、ボタンにして並べる(段階1の仮の入り口)。
function ProvidedButtons({
  name,
  methods,
  onCall,
}: {
  name: string;
  methods: string[];
  onCall: (name: string, method: string) => void;
}) {
  return (
    <div className="extensions-screen__buttons">
      {methods.map((m) => (
        <button key={m} type="button" className="extensions-screen__approve" onClick={() => onCall(name, m)}>
          {m} を呼ぶ
        </button>
      ))}
      {/* 説明書のprovidesに無い操作は、詩織が断る(EXT-020)ことを確かめる */}
      <button type="button" className="extensions-screen__close" onClick={() => onCall(name, "nope")}>
        説明書に無い操作を呼ぶ
      </button>
    </div>
  );
}

export default ExtensionsScreen;
