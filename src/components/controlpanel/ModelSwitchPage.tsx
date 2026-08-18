import { useEffect, useState } from "react";
import { api } from "../../api/tauri";
import type { ModelInfo } from "../../types";
import { formatGb } from "./shared";

type SwitchPhase = "idle" | "estimating" | "confirm" | "switching" | "done" | "error";

// LLMモデルの切替。危険度が最も高いページのため:
// - 切替前に必ずVRAM見込みを提示し、明示確認を挟んでから実行する
// - 危険水域(80%目安)が見込まれる場合は警告を強める
// - 起動失敗時は自動で元のモデルへロールバックする(バックエンド側で実施)
// - 切替中は他のページへ移動できないようにする(ControlPanel側でナビを無効化)
export function ModelSwitchPage({
  onSwitchingChange,
  platform,
}: {
  onSwitchingChange: (v: boolean) => void;
  platform: string | null;
}) {
  // Mac(統合メモリ)はVRAMという概念が無いため、表示文言だけ「メモリ」に差し替える
  // (APIのフィールド名自体はvram*のままだが、意味的には統合メモリ使用量を指す)。
  const memLabel = platform === "macos" ? "メモリ" : "VRAM";
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [target, setTarget] = useState<ModelInfo | null>(null);
  const [phase, setPhase] = useState<SwitchPhase>("idle");
  const [projectedPercent, setProjectedPercent] = useState<number | null>(null);
  const [isRisky, setIsRisky] = useState(false);
  const [resultMessage, setResultMessage] = useState<string | null>(null);

  const loadModels = () => {
    api
      .listAvailableModels()
      .then(setModels)
      .catch((err) => setLoadError(String(err)));
  };

  useEffect(loadModels, []);

  useEffect(() => {
    onSwitchingChange(phase === "switching");
  }, [phase, onSwitchingChange]);

  const handleSelectTarget = async (model: ModelInfo) => {
    setTarget(model);
    setPhase("estimating");
    setResultMessage(null);
    try {
      const estimate = await api.estimateModelSwitch(model.fileName);
      setProjectedPercent(estimate.projectedVramPercent);
      setIsRisky(estimate.isRisky);
      setPhase("confirm");
    } catch (err) {
      setResultMessage(String(err));
      setPhase("error");
    }
  };

  const handleCancel = () => {
    setTarget(null);
    setPhase("idle");
  };

  const handleConfirmSwitch = async () => {
    if (!target) return;
    setPhase("switching");
    try {
      const result = await api.switchModel(target.fileName);
      if (result.success) {
        setResultMessage(
          `切り替えが完了しました(実測${memLabel}: ${result.measuredVramGb ? formatGb(result.measuredVramGb) : "不明"})`,
        );
      } else {
        setResultMessage(
          result.rolledBack
            ? `切替に失敗したため、元のモデルに戻しました: ${result.error}`
            : `切替に失敗し、ロールバックにも失敗しました。手動で確認してください: ${result.error}`,
        );
      }
      setPhase("done");
      setTarget(null);
      loadModels();
    } catch (err) {
      setResultMessage(String(err));
      setPhase("error");
    }
  };

  return (
    <>
      <div className="control-panel__page-head">
        <h1 className="control-panel__page-head-danger">モデル切替</h1>
        <p>この操作は詩織の「答え方そのもの」を切り替えます。切替中は一時的に会話できなくなります。</p>
      </div>

      {loadError && <div className="control-panel__error">{loadError}</div>}
      {resultMessage && phase === "done" && (
        <div className="control-panel__note control-panel__note--safe">{resultMessage}</div>
      )}
      {resultMessage && phase === "error" && (
        <div className="control-panel__note control-panel__note--danger">{resultMessage}</div>
      )}

      <div className="control-panel__panel control-panel__panel--danger">
        <div className="control-panel__danger-warn">
          ⚠ 切替中は一時的に会話ができなくなります。{memLabel}不足の場合は自動的に元のモデルへ
          ロールバックしますが、うまくいかない場合は変更しないことをおすすめします。
        </div>

        {models.map((model) => (
          <div
            key={model.fileName}
            className={`control-panel__model-card${model.isCurrent ? " control-panel__model-card--current" : ""}`}
          >
            <div>
              <div className="control-panel__model-name">
                {model.fileName}
                {model.isCurrent && <span className="control-panel__badge control-panel__badge--safe">使用中</span>}
              </div>
              <div className="control-panel__model-meta">
                {memLabel}目安 {formatGb(model.vramEstimateGb)}
                {model.isMeasured ? "(実測)" : "(概算・未計測)"} ／ ファイルサイズ{" "}
                {(model.sizeMb / 1024).toFixed(1)}GB
              </div>
            </div>
            {!model.isCurrent && (
              <button
                className="control-panel__btn control-panel__btn--danger"
                onClick={() => handleSelectTarget(model)}
                disabled={phase === "estimating" || phase === "switching"}
              >
                このモデルに切り替える
              </button>
            )}
          </div>
        ))}

        {target && phase === "confirm" && (
          <div
            className={`control-panel__note ${isRisky ? "control-panel__note--danger" : "control-panel__note--caution"}`}
          >
            {isRisky ? "⚠ " : ""}
            {target.fileName}に切り替えると、{memLabel}使用率はおよそ{" "}
            <strong>{projectedPercent?.toFixed(0)}%</strong> になる見込みです。
            {isRisky && "危険水域(80%以上)に達する可能性があります。"}
            <div className="control-panel__actions">
              <button className="control-panel__btn" onClick={handleCancel}>
                キャンセル
              </button>
              <button className="control-panel__btn control-panel__btn--danger" onClick={handleConfirmSwitch}>
                {isRisky ? "リスクを理解した上で切り替える" : "切り替える"}
              </button>
            </div>
          </div>
        )}

        {phase === "switching" && (
          <div className="control-panel__note control-panel__note--caution">
            モデルを切り替えています... しばらくお待ちください(数十秒かかることがあります)
          </div>
        )}

        <div className="control-panel__restart-note">
          ⚠ 切替には数十秒かかります。切替前に現在のVRAM状況を「システム状態」で確認しておくことをおすすめします。
        </div>
      </div>
    </>
  );
}
