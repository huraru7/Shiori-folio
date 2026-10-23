import { useEffect } from "react";
import { useExtensionsStore } from "../../store/useExtensionsStore";
import type { ExtensionInfo } from "../../types";
import "./ExtensionsScreen.css";

const STATUS_LABEL: Record<ExtensionInfo["status"], string> = {
  ready: "使える",
  "needs-approval": "未承認",
  invalid: "読めない",
};

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

  useEffect(() => {
    if (list === null) load();
  }, [list, load]);

  return (
    <div className="extensions-screen">
      <p className="extensions-screen__lead">
        portable/extensions/ に置いた拡張機能です。詩織の起動時に読み込みます。足したり外したりしたら、詩織を起動し直してください。
      </p>
      {error && <div className="extensions-screen__error">{error}</div>}
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

export default ExtensionsScreen;
