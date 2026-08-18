import { useEffect, useState } from "react";
import { api } from "../../api/tauri";
import type { AppConfigDto } from "../../types";
import { NumberRow, SliderRow, TextRow, ToggleRow } from "./shared";

type FieldKey =
  | "hotkey"
  | "llmPort"
  | "llmContextSize"
  | "embeddingPort"
  | "sttPort"
  | "ragPort"
  | "passiveRecallThreshold"
  | "showYear"
  | "showMonth"
  | "showDay"
  | "showWeekday";

// 変更内容によって「即時反映」「サービス再起動が必要」「アプリ再起動が必要」が
// 異なるため、フィールドごとにどちらに属するかを持たせておく。
const RESTART_KIND: Record<FieldKey, "immediate" | "llm-restart" | "app-restart" | "manual-rag-restart"> = {
  hotkey: "app-restart",
  llmPort: "llm-restart",
  llmContextSize: "llm-restart",
  embeddingPort: "llm-restart",
  sttPort: "immediate",
  ragPort: "manual-rag-restart",
  passiveRecallThreshold: "immediate",
  showYear: "immediate",
  showMonth: "immediate",
  showDay: "immediate",
  showWeekday: "immediate",
};

// ポート番号・ホットキー・コンテキストサイズ・自発的想起のしきい値を扱うページ。
// 保存はconfig.jsonへの書き込みのみ即座に行い、実際にサービスへ反映するかどうかは
// ユーザーの明示操作(再起動ボタン)に委ねる(意図せずサービスが落ちるのを防ぐため)。
export function AdvancedSettingsPage() {
  const [original, setOriginal] = useState<AppConfigDto | null>(null);
  const [form, setForm] = useState<AppConfigDto | null>(null);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [restarting, setRestarting] = useState(false);

  useEffect(() => {
    api
      .getConfig()
      .then((config) => {
        setOriginal(config);
        setForm(config);
      })
      .catch((err) => setErrorMessage(String(err)));
  }, []);

  if (!form || !original) {
    return (
      <div className="control-panel__page-head">
        <h1>詳細設定</h1>
        <p>読み込み中...</p>
      </div>
    );
  }

  const changedKeys = (Object.keys(RESTART_KIND) as FieldKey[]).filter(
    (key) => form[key] !== original[key],
  );
  const needsAppRestart = changedKeys.some((k) => RESTART_KIND[k] === "app-restart");
  const needsLlmRestart = changedKeys.some((k) => RESTART_KIND[k] === "llm-restart");
  const needsManualRagRestart = changedKeys.some((k) => RESTART_KIND[k] === "manual-rag-restart");

  const handleSave = async () => {
    setSaveState("saving");
    setErrorMessage(null);
    try {
      const update: Partial<AppConfigDto> = {
        ...(form.hotkey !== original.hotkey && { hotkey: form.hotkey }),
        ...(form.llmPort !== original.llmPort && { llmPort: form.llmPort }),
        ...(form.llmContextSize !== original.llmContextSize && {
          llmContextSize: form.llmContextSize,
        }),
        ...(form.embeddingPort !== original.embeddingPort && {
          embeddingPort: form.embeddingPort,
        }),
        ...(form.sttPort !== original.sttPort && { sttPort: form.sttPort }),
        ...(form.ragPort !== original.ragPort && { ragPort: form.ragPort }),
        ...(form.passiveRecallThreshold !== original.passiveRecallThreshold && {
          passiveRecallThreshold: form.passiveRecallThreshold,
        }),
        ...(form.showYear !== original.showYear && { showYear: form.showYear }),
        ...(form.showMonth !== original.showMonth && { showMonth: form.showMonth }),
        ...(form.showDay !== original.showDay && { showDay: form.showDay }),
        ...(form.showWeekday !== original.showWeekday && { showWeekday: form.showWeekday }),
      };
      await api.setConfig(update);
      setOriginal(form);
      setSaveState("saved");
    } catch (err) {
      setSaveState("error");
      setErrorMessage(String(err));
    }
  };

  const handleRestartLlm = async () => {
    setRestarting(true);
    try {
      await api.restartLlmServices();
    } catch (err) {
      setErrorMessage(String(err));
    } finally {
      setRestarting(false);
    }
  };

  const handleRestartApp = () => {
    api.restartApp().catch((err) => setErrorMessage(String(err)));
  };

  return (
    <>
      <div className="control-panel__page-head">
        <h1>詳細設定</h1>
        <p>保存すると値はconfig.jsonに書き込まれます。反映方法は項目によって異なります。</p>
      </div>

      {errorMessage && <div className="control-panel__error">{errorMessage}</div>}

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">ホットキー</div>
        <TextRow
          label="録音開始/停止のホットキー"
          value={form.hotkey}
          onChange={(v) => setForm({ ...form, hotkey: v })}
        />
      </div>

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">ポート番号</div>
        <NumberRow label="LLM(llama-server)" value={form.llmPort} onChange={(v) => setForm({ ...form, llmPort: v })} />
        <NumberRow
          label="Embedding(llama-server)"
          value={form.embeddingPort}
          onChange={(v) => setForm({ ...form, embeddingPort: v })}
        />
        <NumberRow
          label="STT(whisper-server)"
          value={form.sttPort}
          onChange={(v) => setForm({ ...form, sttPort: v })}
        />
        <NumberRow
          label="RAG検索サーバー(Python)"
          value={form.ragPort}
          onChange={(v) => setForm({ ...form, ragPort: v })}
        />
      </div>

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">LLM・自発的想起</div>
        <NumberRow
          label="コンテキストサイズ(--ctx-size)"
          value={form.llmContextSize ?? 8192}
          onChange={(v) => setForm({ ...form, llmContextSize: v })}
        />
        <SliderRow
          label="常時軽量検索の類似度しきい値"
          value={form.passiveRecallThreshold}
          min={0}
          max={1.2}
          step={0.01}
          onChange={(v) => setForm({ ...form, passiveRecallThreshold: v })}
        />
      </div>

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">ヘッダーの日付表示</div>
        <ToggleRow label="年を表示" checked={form.showYear} onChange={(v) => setForm({ ...form, showYear: v })} />
        <ToggleRow label="月を表示" checked={form.showMonth} onChange={(v) => setForm({ ...form, showMonth: v })} />
        <ToggleRow label="日を表示" checked={form.showDay} onChange={(v) => setForm({ ...form, showDay: v })} />
        <ToggleRow
          label="曜日を表示"
          checked={form.showWeekday}
          onChange={(v) => setForm({ ...form, showWeekday: v })}
        />
      </div>

      <div className="control-panel__actions">
        <button
          className="control-panel__btn control-panel__btn--primary"
          onClick={handleSave}
          disabled={saveState === "saving" || changedKeys.length === 0}
        >
          {saveState === "saving" ? "保存中..." : "変更を保存"}
        </button>
      </div>

      {saveState === "saved" && needsAppRestart && (
        <div className="control-panel__note control-panel__note--danger">
          ⚠ ホットキーの変更はアプリの再起動後に反映されます
          <button className="control-panel__btn" onClick={handleRestartApp}>
            アプリを再起動
          </button>
        </div>
      )}
      {saveState === "saved" && needsLlmRestart && (
        <div className="control-panel__note control-panel__note--caution">
          ⚠ ポート/コンテキストサイズの変更はサービス再起動後に反映されます
          <button className="control-panel__btn" onClick={handleRestartLlm} disabled={restarting}>
            {restarting ? "再起動中..." : "LLMサービスを再起動"}
          </button>
        </div>
      )}
      {saveState === "saved" && needsManualRagRestart && (
        <div className="control-panel__note control-panel__note--caution">
          ⚠ RAGサーバー(Python)はこのアプリの管理外のため、手動で再起動してください
        </div>
      )}
      {saveState === "saved" && !needsAppRestart && !needsLlmRestart && !needsManualRagRestart && (
        <div className="control-panel__note control-panel__note--safe">即時反映されました</div>
      )}
    </>
  );
}
