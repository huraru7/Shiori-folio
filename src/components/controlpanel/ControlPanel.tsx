import { useEffect, useState } from "react";
import { api } from "../../api/tauri";
import type { AppConfigDto, ModelInfo, SystemInfo } from "../../types";
import "./ControlPanel.css";

type NavKey = "system" | "voice" | "advanced" | "model";

const NAV_ITEMS: { key: NavKey; label: string; level: "safe" | "caution" | "danger" }[] = [
  { key: "system", label: "システム状態", level: "safe" },
  { key: "voice", label: "音声設定", level: "safe" },
  { key: "advanced", label: "詳細設定", level: "caution" },
  { key: "model", label: "モデル切替", level: "danger" },
];

// 使用率に応じた3段階のレベル分け(60%未満=安全、60〜79%=要注意、80%以上=危険)。
// パネル本文の色使いを静かに保つため、しきい値を超えたときだけ色を変える。
function levelFor(percent: number): "safe" | "caution" | "danger" {
  if (percent >= 80) return "danger";
  if (percent >= 60) return "caution";
  return "safe";
}

// GPU温度用。ノートPCはサーマルスロットリングが起きやすいため80℃を危険の目安にする。
function levelForTemp(celsius: number): "safe" | "caution" | "danger" {
  if (celsius >= 80) return "danger";
  if (celsius >= 70) return "caution";
  return "safe";
}

function formatMb(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)}GB`;
  return `${mb}MB`;
}

const HISTORY_LENGTH = 200; // 1.5秒間隔ポーリングで約5分ぶん

// 直近の推移を示す簡易スパークライン。SVGのpolylineで最小限の描画に留める。
function Sparkline({ values, max }: { values: number[]; max: number }) {
  if (values.length < 2) return null;
  const width = 100;
  const height = 24;
  const points = values
    .map((v, i) => {
      const x = (i / (values.length - 1)) * width;
      const y = height - (Math.min(v, max) / max) * height;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <svg className="control-panel__sparkline" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none">
      <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

// 設定ウィンドウ(2026-08-12、デスクトップ型ウィンドウシステムの本実装3-3)。
// 以前はスタンドアロンの全画面ビューだったが、OsWindow内のコンパクト版に
// 作り替えた(開閉・ドラッグ・リサイズ等はOsWindow側が担うため、ここでは
// 左ナビ+右コンテンツの2ペイン構造だけを持つ)。
// 優先度1(システムモニター、読み取り専用)のみ実装済み。他のナビ項目は準備中。
function ControlPanel() {
  const [activeNav, setActiveNav] = useState<NavKey>("system");
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  // モデル切替の実行中は、途中で画面を離れられると状態把握が難しくなるため
  // サイドバーのナビゲーション・会話画面への「戻る」を一時的に無効化する。
  const [isSwitchingModel, setIsSwitchingModel] = useState(false);

  // 瞬間的なピーク(録音中だけVRAMが跳ねる等)を見逃さないための直近推移バッファ。
  // グラフ描画ライブラリは使わず、配列をそのままSVGのpolylineに渡す。
  const [vramHistory, setVramHistory] = useState<number[]>([]);
  const [ramHistory, setRamHistory] = useState<number[]>([]);
  const [tempHistory, setTempHistory] = useState<number[]>([]);

  useEffect(() => {
    let cancelled = false;

    const poll = async () => {
      try {
        const result = await api.getSystemInfo();
        if (!cancelled) {
          setInfo(result);
          setError(null);
          if (result.gpu) {
            setVramHistory((h) => [...h, (result.gpu!.vramUsedMb / result.gpu!.vramTotalMb) * 100].slice(-HISTORY_LENGTH));
            if (result.gpu.temperatureC !== null) {
              setTempHistory((h) => [...h, result.gpu!.temperatureC!].slice(-HISTORY_LENGTH));
            }
          }
          setRamHistory((h) => [...h, (result.ramUsedMb / result.ramTotalMb) * 100].slice(-HISTORY_LENGTH));
        }
      } catch (err) {
        if (!cancelled) setError(String(err));
      }
    };

    poll();
    const timer = setInterval(poll, 1500);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  const vramPercent = info?.gpu
    ? Math.round((info.gpu.vramUsedMb / info.gpu.vramTotalMb) * 100)
    : null;
  const ramPercent = info ? Math.round((info.ramUsedMb / info.ramTotalMb) * 100) : null;

  return (
    <div className="control-panel">
      <aside className="control-panel__sidebar">
        {NAV_ITEMS.map((item) => (
          <div
            key={item.key}
            className={`control-panel__nav-item${activeNav === item.key ? " control-panel__nav-item--active" : ""}${isSwitchingModel ? " control-panel__nav-item--disabled" : ""}`}
            onClick={() => !isSwitchingModel && setActiveNav(item.key)}
            title={item.label}
          >
            <span className={`control-panel__dot control-panel__dot--${item.level}`} />
            <span className="control-panel__nav-label">{item.label}</span>
          </div>
        ))}
      </aside>

      <main className="control-panel__main">
        {activeNav === "system" && (
          <SystemStatusPage
            info={info}
            error={error}
            vramPercent={vramPercent}
            ramPercent={ramPercent}
            vramHistory={vramHistory}
            ramHistory={ramHistory}
            tempHistory={tempHistory}
          />
        )}
        {activeNav === "voice" && <VoiceSettingsPage />}
        {activeNav === "advanced" && <AdvancedSettingsPage />}
        {activeNav === "model" && <ModelSwitchPage onSwitchingChange={setIsSwitchingModel} />}
      </main>
    </div>
  );
}

function SystemStatusPage({
  info,
  error,
  vramPercent,
  ramPercent,
  vramHistory,
  ramHistory,
  tempHistory,
}: {
  info: SystemInfo | null;
  error: string | null;
  vramPercent: number | null;
  ramPercent: number | null;
  vramHistory: number[];
  ramHistory: number[];
  tempHistory: number[];
}) {
  const cpuPercent = info ? Math.round(info.cpuUsagePercent) : null;
  const tempLevel = info?.gpu?.temperatureC !== null && info?.gpu?.temperatureC !== undefined
    ? levelForTemp(info.gpu.temperatureC)
    : "safe";

  return (
    <>
      <div className="control-panel__page-head">
        <h1>システム状態</h1>
        <p>読み取り専用です。ここでは何も変更されません。</p>
      </div>

      {error && <div className="control-panel__error">取得に失敗しました: {error}</div>}

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">リソース使用状況</div>
        <div className="control-panel__gauge-row">
          <div className="control-panel__gauge">
            <div className="control-panel__gauge-top">
              <span className="control-panel__gauge-name">VRAM</span>
              {vramPercent !== null && (
                <span className={`control-panel__gauge-val control-panel__gauge-val--${levelFor(vramPercent)}`}>
                  {vramPercent}%
                </span>
              )}
            </div>
            {info?.gpu ? (
              <>
                <div className="control-panel__gauge-bar">
                  <div
                    className={`control-panel__gauge-fill control-panel__gauge-fill--${levelFor(vramPercent ?? 0)}`}
                    style={{ width: `${vramPercent}%` }}
                  />
                </div>
                <div className="control-panel__gauge-sub">
                  {formatMb(info.gpu.vramUsedMb)} / {formatMb(info.gpu.vramTotalMb)}
                </div>
                <div className={`control-panel__gauge-val--${levelFor(vramPercent ?? 0)}`}>
                  <Sparkline values={vramHistory} max={100} />
                </div>
              </>
            ) : (
              <div className="control-panel__gauge-sub">GPU情報を取得できません(nvidia-smi未検出)</div>
            )}
          </div>

          <div className="control-panel__gauge">
            <div className="control-panel__gauge-top">
              <span className="control-panel__gauge-name">システムRAM</span>
              {ramPercent !== null && (
                <span className={`control-panel__gauge-val control-panel__gauge-val--${levelFor(ramPercent)}`}>
                  {ramPercent}%
                </span>
              )}
            </div>
            <div className="control-panel__gauge-bar">
              <div
                className={`control-panel__gauge-fill control-panel__gauge-fill--${levelFor(ramPercent ?? 0)}`}
                style={{ width: `${ramPercent ?? 0}%` }}
              />
            </div>
            <div className="control-panel__gauge-sub">
              {info ? `${formatMb(info.ramUsedMb)} / ${formatMb(info.ramTotalMb)}` : "取得中..."}
            </div>
            <div className={`control-panel__gauge-val--${levelFor(ramPercent ?? 0)}`}>
              <Sparkline values={ramHistory} max={100} />
            </div>
          </div>

          <div className="control-panel__gauge">
            <div className="control-panel__gauge-top">
              <span className="control-panel__gauge-name">CPU</span>
              {cpuPercent !== null && (
                <span className={`control-panel__gauge-val control-panel__gauge-val--${levelFor(cpuPercent)}`}>
                  {cpuPercent}%
                </span>
              )}
            </div>
            <div className="control-panel__gauge-bar">
              <div
                className={`control-panel__gauge-fill control-panel__gauge-fill--${levelFor(cpuPercent ?? 0)}`}
                style={{ width: `${cpuPercent ?? 0}%` }}
              />
            </div>
            <div className="control-panel__gauge-sub">{info?.cpu.model ?? "取得中..."}</div>
          </div>

          <div className="control-panel__gauge">
            <div className="control-panel__gauge-top">
              <span className="control-panel__gauge-name">GPU温度</span>
              {info?.gpu?.temperatureC != null && (
                <span className={`control-panel__gauge-val control-panel__gauge-val--${tempLevel}`}>
                  {info.gpu.temperatureC}℃
                </span>
              )}
            </div>
            <div className="control-panel__gauge-sub">
              {info?.gpu?.temperatureC == null ? "取得できません" : "80℃以上で注意"}
            </div>
            <div className={`control-panel__gauge-val--${tempLevel}`}>
              <Sparkline values={tempHistory} max={100} />
            </div>
          </div>
        </div>

        {info?.diskThroughput && (
          <div className="control-panel__disk-io">
            ディスク I/O(
            {info.storage?.drive ?? "動作ドライブ"}): 読み込み {info.diskThroughput.readMbPerSec.toFixed(1)}MB/s ／
            書き込み {info.diskThroughput.writeMbPerSec.toFixed(1)}MB/s
          </div>
        )}
      </div>

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">サービス稼働状況</div>
        <div className="control-panel__service-list">
          {(info?.services ?? []).map((service) => (
            <div className="control-panel__service-item" key={service.name}>
              <span>
                {service.name}
                {service.modelName && (
                  <span className="control-panel__service-model"> · {service.modelName}</span>
                )}
              </span>
              <span className="control-panel__service-status">
                {service.running && <span className="control-panel__pulse" />}
                {service.running ? "稼働中" : "待機(オンデマンド)"}
                <span className="control-panel__service-usage">
                  VRAM {service.vramMb !== null ? formatMb(service.vramMb) : "—"} / RAM{" "}
                  {service.ramMb !== null ? formatMb(service.ramMb) : "—"}
                </span>
              </span>
            </div>
          ))}
        </div>
      </div>

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">PC本体情報</div>
        <div className="control-panel__pc-info">
          <div>OS：{info?.os ?? "取得中..."}</div>
          <div>
            CPU：{info?.cpu.model ?? "取得中..."}
            {info && ` (物理${info.cpu.physicalCores}コア／論理${info.cpu.logicalCores}スレッド／${(info.cpu.frequencyMhz / 1000).toFixed(1)}GHz)`}
          </div>
          {info?.gpu && <div>GPU：{info.gpu.name}</div>}

          {info?.storage && (
            <div>
              ストレージ：{info.storage.drive}（{info.storage.kind}
              {info.storage.isRemovable ? "・リムーバブル(USB等)" : ""}／{info.storage.fileSystem}） 空き{" "}
              {info.storage.freeGb.toFixed(1)}GB / 総容量 {info.storage.totalGb.toFixed(1)}GB
            </div>
          )}

          {info?.power && (
            <div>
              電源：
              {info.power.onBattery
                ? `バッテリー駆動${info.power.batteryPercent !== null ? `(残量${info.power.batteryPercent.toFixed(0)}%)` : ""}`
                : "AC電源接続中"}
            </div>
          )}
        </div>
      </div>
    </>
  );
}

const PREVIEW_TEXT = "こんにちは、詩織です。今の設定で話すとこんな感じになります。";

// 発話パラメータ(速度・ノイズ)を試しながら調整するページ。
// 「試し読み」は保存前の値でその場で再生するだけ(config.jsonには書き込まない)。
// 保存は別ボタンで、piper-plusは呼び出しのたびに引数を渡すだけの設計のため
// サービス再起動は不要(即時反映)。
function VoiceSettingsPage() {
  const [lengthScale, setLengthScale] = useState(1);
  const [noiseScale, setNoiseScale] = useState(0.667);
  const [noiseW, setNoiseW] = useState(0.8);
  const [loaded, setLoaded] = useState(false);
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    api
      .getConfig()
      .then((config) => {
        setLengthScale(config.lengthScale);
        setNoiseScale(config.noiseScale);
        setNoiseW(config.noiseW);
        setLoaded(true);
      })
      .catch((err) => setErrorMessage(String(err)));
  }, []);

  const handlePreview = async () => {
    setIsPreviewing(true);
    setErrorMessage(null);
    try {
      await api.previewVoice(PREVIEW_TEXT, lengthScale, noiseScale, noiseW);
    } catch (err) {
      setErrorMessage(String(err));
    } finally {
      setIsPreviewing(false);
    }
  };

  const handleSave = async () => {
    setSaveState("saving");
    setErrorMessage(null);
    try {
      await api.setConfig({ lengthScale, noiseScale, noiseW });
      setSaveState("saved");
    } catch (err) {
      setSaveState("error");
      setErrorMessage(String(err));
    }
  };

  return (
    <>
      <div className="control-panel__page-head">
        <h1>音声設定</h1>
        <p>発話パラメータを調整できます。保存すると次の発話から即時反映されます(再起動不要)。</p>
      </div>

      {errorMessage && <div className="control-panel__error">{errorMessage}</div>}

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">発話パラメータ</div>

        <SliderRow
          label="発話速度(length-scale)"
          value={lengthScale}
          min={0.5}
          max={2}
          step={0.05}
          onChange={setLengthScale}
        />
        <SliderRow
          label="声の揺らぎ(noise-scale)"
          value={noiseScale}
          min={0}
          max={1.5}
          step={0.01}
          onChange={setNoiseScale}
        />
        <SliderRow
          label="音素幅ノイズ(noise-w)"
          value={noiseW}
          min={0}
          max={1.5}
          step={0.01}
          onChange={setNoiseW}
        />

        <div className="control-panel__actions">
          <button className="control-panel__btn" onClick={handlePreview} disabled={!loaded || isPreviewing}>
            {isPreviewing ? "再生中..." : "試し読み再生"}
          </button>
          <button
            className="control-panel__btn control-panel__btn--primary"
            onClick={handleSave}
            disabled={!loaded || saveState === "saving"}
          >
            {saveState === "saving" ? "保存中..." : "保存して反映"}
          </button>
        </div>

        {saveState === "saved" && (
          <div className="control-panel__note control-panel__note--safe">即時反映されました(再起動は不要です)</div>
        )}
      </div>
    </>
  );
}

function SliderRow({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  return (
    <div className="control-panel__slider-row">
      <div className="control-panel__slider-top">
        <span>{label}</span>
        <span className="control-panel__slider-val">{value.toFixed(2)}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="control-panel__slider-input"
      />
    </div>
  );
}

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
function AdvancedSettingsPage() {
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

function TextRow({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="control-panel__field-row">
      <span className="control-panel__field-label">{label}</span>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="control-panel__text-input"
      />
    </div>
  );
}

function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="control-panel__field-row">
      <span className="control-panel__field-label">{label}</span>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </div>
  );
}

function NumberRow({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
}) {
  return (
    <div className="control-panel__field-row">
      <span className="control-panel__field-label">{label}</span>
      <input
        type="number"
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="control-panel__text-input"
      />
    </div>
  );
}

function formatGb(gb: number): string {
  return `${gb.toFixed(1)}GB`;
}

type SwitchPhase = "idle" | "estimating" | "confirm" | "switching" | "done" | "error";

// LLMモデルの切替。危険度が最も高いページのため:
// - 切替前に必ずVRAM見込みを提示し、明示確認を挟んでから実行する
// - 危険水域(80%目安)が見込まれる場合は警告を強める
// - 起動失敗時は自動で元のモデルへロールバックする(バックエンド側で実施)
// - 切替中は他のページへ移動できないようにする(ControlPanel側でナビを無効化)
function ModelSwitchPage({ onSwitchingChange }: { onSwitchingChange: (v: boolean) => void }) {
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
          `切り替えが完了しました(実測VRAM: ${result.measuredVramGb ? formatGb(result.measuredVramGb) : "不明"})`,
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
          ⚠ 切替中は一時的に会話ができなくなります。VRAM不足の場合は自動的に元のモデルへ
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
                VRAM目安 {formatGb(model.vramEstimateGb)}
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
            {target.fileName}に切り替えると、VRAM使用率はおよそ{" "}
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

export default ControlPanel;
