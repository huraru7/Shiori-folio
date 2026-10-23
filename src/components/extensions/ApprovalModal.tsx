import { useEffect } from "react";
import { useExtensionsStore } from "../../store/useExtensionsStore";
import "../main/PendingModal.css";
import "./ApprovalModal.css";

// 拡張機能の承認の画面(詩織Ver6.0の段階1)。起動時に、初めての拡張機能と、使うものが増えた
// 拡張機能を1つずつ出し、「この拡張機能は◯◯を使います」と見せて承認を取る。
// 「今回は読み込まない」を選んだものは、この起動の間は聞き直さない(拡張機能のタブから承認できる)。
// 見た目は要確認の画面(PendingModal)の枠を借りた仮のもので、段階2でデザインを合わせる。
function ApprovalModal() {
  const list = useExtensionsStore((s) => s.list);
  const skipped = useExtensionsStore((s) => s.skipped);
  const load = useExtensionsStore((s) => s.load);
  const approve = useExtensionsStore((s) => s.approve);
  const skip = useExtensionsStore((s) => s.skip);
  const error = useExtensionsStore((s) => s.error);

  useEffect(() => {
    load();
  }, [load]);

  const target = list?.find((e) => e.status === "needs-approval" && e.name && !skipped.includes(e.name));
  if (!target || !target.name) return null;
  const name = target.name;

  return (
    <div className="pending-modal__backdrop">
      <div className="pending-modal approval-modal" role="dialog" aria-modal="true" aria-label="拡張機能の承認">
        <div className="pending-modal__header">
          <span className="pending-modal__title">拡張機能の承認</span>
        </div>
        <div className="approval-modal__body">
          <p className="approval-modal__name">
            {target.title}
            <span className="approval-modal__meta">
              {target.folder} / 版 {target.version}
            </span>
          </p>
          {target.description && <p className="approval-modal__desc">{target.description}</p>}
          <p className="approval-modal__lead">
            {target.firstTime
              ? "初めての拡張機能です。この拡張機能は、次のものを使います。"
              : "この拡張機能が使うものが増えました。新しく、次のものを使います。"}
          </p>
          {target.addedUses.length === 0 ? (
            <p className="approval-modal__none">詩織から借りるものはありません(ログとエラーの報告だけ)。</p>
          ) : (
            <ul className="approval-modal__uses">
              {target.addedUses.map((u) => (
                <li key={u.name}>
                  {u.label}
                  <span className="approval-modal__use-name">{u.name}</span>
                </li>
              ))}
            </ul>
          )}
          {error && <p className="approval-modal__error">{error}</p>}
        </div>
        <div className="approval-modal__actions">
          <button type="button" className="approval-modal__btn" onClick={() => skip(name)}>
            今回は読み込まない
          </button>
          <button
            type="button"
            className="approval-modal__btn approval-modal__btn--primary"
            onClick={() => approve(name)}
          >
            承認する
          </button>
        </div>
      </div>
    </div>
  );
}

export default ApprovalModal;
