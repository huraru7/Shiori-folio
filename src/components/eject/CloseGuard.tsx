import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../../api/tauri";
import type { EjectPreview } from "../../types";
import EjectSummary from "./EjectSummary";
import "./Eject.css";

// 詩織のウィンドウを閉じるときの確認。詩織は終了時に、自分が起動した子プロセスだけを止める。
// MCPや別のセッションが起動した共有デーモン(RAG・埋め込み)は残り、SSDの取り外しを妨げる。
// そうしたものが動いているときだけ、「関連するプロセスも止めますか?」と聞く。
// Cmd+Q・Dockからの終了は、ウィンドウを閉じる操作ではないため、この確認を通らない。
function CloseGuard() {
  const [preview, setPreview] = useState<EjectPreview | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    win
      .onCloseRequested(async (event) => {
        event.preventDefault();
        try {
          const p = await api.ejectPreview();
          if (p.leftoverAfterExit.length === 0) {
            await win.destroy();
            return;
          }
          setPreview(p);
        } catch {
          // 確認に失敗しても、閉じられなくなるのは避ける。
          await win.destroy();
        }
      })
      .then((u) => {
        unlisten = u;
      });
    return () => unlisten?.();
  }, []);

  if (!preview) return null;

  const closeOnly = () => getCurrentWindow().destroy();
  const stopAndClose = async () => {
    setBusy(true);
    try {
      await api.ejectExecute(); // 止めたあと、詩織は自分で終了する
    } catch {
      await closeOnly();
    }
  };

  return (
    <div className="eject-guard__backdrop">
      <div className="eject-guard">
        <h2 className="eject-guard__title">関連するプロセスも停止しますか?</h2>
        <p className="eject-guard__lead">
          詩織を閉じても、次のプロセスはSSD上で動き続けます。SSDを取り外すなら、止めておくと安全です。
        </p>
        <EjectSummary preview={{ ...preview, plan: { ...preview.plan, toStop: preview.leftoverAfterExit } }} />
        <div className="eject__actions">
          <button className="control-panel__btn control-panel__btn--primary" onClick={stopAndClose} disabled={busy}>
            {busy ? "停止しています…" : "止めて閉じる"}
          </button>
          <button className="control-panel__btn" onClick={closeOnly} disabled={busy}>
            そのまま閉じる
          </button>
          <button className="control-panel__btn" onClick={() => setPreview(null)} disabled={busy}>
            キャンセル
          </button>
        </div>
      </div>
    </div>
  );
}

export default CloseGuard;
