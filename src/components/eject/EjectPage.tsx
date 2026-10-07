import { useCallback, useEffect, useState } from "react";
import { api } from "../../api/tauri";
import type { EjectOutcome, EjectPreview } from "../../types";
import EjectSummary from "./EjectSummary";
import "./Eject.css";

// 設定画面の「SSDの取り外し」ページ。実機SSDを使っているプロセスを確認し、まとめて止めて
// 詩織を終了する。止める対象の選び方と、止めないもの(mcp_server)の理由は eject.rs を参照。
export function EjectPage() {
  const [preview, setPreview] = useState<EjectPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<EjectOutcome | null>(null);

  const load = useCallback(() => {
    setError(null);
    api
      .ejectPreview()
      .then(setPreview)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(load, [load]);

  const execute = async () => {
    setBusy(true);
    try {
      setOutcome(await api.ejectExecute());
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <>
      <div className="control-panel__page-head">
        <h1>SSDの取り外し</h1>
        <p>実機のSSDを使っているプロセスを止めて、取り外せる状態にします。取り外し自体(Finderで取り出す操作)は行いません。</p>
      </div>

      {error && <div className="control-panel__error">失敗しました: {error}</div>}

      <div className="control-panel__panel">
        {preview ? <EjectSummary preview={preview} /> : !error && <p>確認しています…(数秒かかります)</p>}

        {outcome ? (
          <div className={`eject__result${outcome.failed.length ? " eject__result--error" : ""}`}>
            {outcome.failed.length
              ? `止められなかったプロセスがあります(pid ${outcome.failed.join(", ")})。詩織を終了します。`
              : "関連するプロセスを止めました。詩織を終了します。"}
            {preview && preview.plan.inUse.length > 0 &&
              " 使用中のもの(Claudeのセッションが使うmcp_server)は、セッションを閉じてから、Finderで取り出してください。"}
          </div>
        ) : (
          <div className="eject__actions">
            <button className="control-panel__btn" onClick={load} disabled={busy}>
              再確認
            </button>
            {!confirming ? (
              <button
                className="control-panel__btn control-panel__btn--primary"
                onClick={() => setConfirming(true)}
                disabled={busy || !preview}
              >
                止めて詩織を終了する
              </button>
            ) : (
              <>
                <button
                  className="control-panel__btn control-panel__btn--danger"
                  onClick={execute}
                  disabled={busy}
                >
                  {busy ? "停止しています…" : "本当に止めて終了する"}
                </button>
                <button className="control-panel__btn" onClick={() => setConfirming(false)} disabled={busy}>
                  やめる
                </button>
              </>
            )}
          </div>
        )}
      </div>
    </>
  );
}
