import { useEffect, useState } from "react";
import { api } from "../../api/tauri";
import type { SystemInfo } from "../../types";
import "./ControlPanel.css";
import { AdvancedSettingsPage } from "./AdvancedSettingsPage";
import { ModelSwitchPage } from "./ModelSwitchPage";
import { SystemStatusPage } from "./SystemStatusPage";
import { VoiceSettingsPage } from "./VoiceSettingsPage";
import { HISTORY_LENGTH } from "./shared";

type NavKey = "system" | "voice" | "advanced" | "model";

const NAV_ITEMS: { key: NavKey; label: string; level: "safe" | "caution" | "danger" }[] = [
  { key: "system", label: "システム状態", level: "safe" },
  { key: "voice", label: "音声設定", level: "safe" },
  { key: "advanced", label: "詳細設定", level: "caution" },
  { key: "model", label: "モデル切替", level: "danger" },
];

// 設定ウィンドウ(2026-08-12、デスクトップ型ウィンドウシステムの本実装3-3)。
// 以前はスタンドアロンの全画面ビューだったが、OsWindow内のコンパクト版に
// 作り替えた(開閉・ドラッグ・リサイズ等はOsWindow側が担うため、ここでは
// 左ナビ+右コンテンツの2ペイン構造だけを持つ)。
//
// 【重要】DesktopArea/OsWindowは常時マウント方式のため(StartupScreenは
// 単なるCSSオーバーレイであり、その裏でこのコンポーネントもアプリ起動直後から
// マウントされている)、バックエンド起動(RAG/LLM等)を待たずにマウント時の
// useEffectが走る。ここでのgetSystemInfoはRust側ネイティブAPIのみに依存し
// サイドカー起動を待つ必要が無いため今は問題ないが、RAG/LLM依存の新規fetchを
// このコンポーネントや配下のページに追加する場合は、LibraryScreen.tsxと同様に
// useShioriStore.getState().ensureBackendServicesStarted()の完了を待ってから
// 呼ぶこと(でないと起動直後は必ず失敗する)。
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
        {activeNav === "model" && (
          <ModelSwitchPage onSwitchingChange={setIsSwitchingModel} platform={info?.platform ?? null} />
        )}
      </main>
    </div>
  );
}

export default ControlPanel;
