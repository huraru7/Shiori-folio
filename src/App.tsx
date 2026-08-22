import { useEffect, useState } from "react";
import TopBar from "./components/TopBar";
import { getLeftCollapsed, setLeftCollapsed } from "./lib/leftCollapsed";
import ActivityIndicator from "./components/ActivityIndicator";
import VoiceBar from "./components/VoiceBar";
import ConversationLog from "./components/ConversationLog";
import CharacterStage from "./components/character/CharacterStage";
import DesktopArea from "./components/desktop/DesktopArea";
import Dock from "./components/desktop/Dock";
import DebugScreen from "./components/debug/DebugScreen";
import StartupScreen from "./components/startup/StartupScreen";
import { useShioriStore } from "./store/useShioriStore";
import "./App.css";

// 全体レイアウト。ヘッダー(全幅)＋本体行(左ゾーン flex:3＝Live2D/オーブ・
// チャット・入力欄、右ゾーン flex:7＝デスクトップ型フリースペース＋Dock)の
// 構成。図書館・設定はデスクトップ型ウィンドウシステムの本実装(2026-08-12)
// により、フリースペース内のOsWindowとして表示される(DesktopArea参照)。
// デバッグのみ、内部が複雑な既存UIのため引き続き独立した全画面表示のまま。
// タスク管理機能(ProjectPanel)は廃止済み(メモ機能に置き換え、2026-08-08)。
// Zustandはグローバルフックのため、Providerは不要。
function App() {
  const [isDebugOpen, setIsDebugOpen] = useState(false);
  const [showStartup, setShowStartup] = useState(true);
  const [leftCollapsed, setLeftCollapsedState] = useState(getLeftCollapsed);

  const toggleLeftCollapsed = () => {
    setLeftCollapsedState((prev) => {
      const next = !prev;
      setLeftCollapsed(next);
      return next;
    });
  };

  useEffect(() => {
    // ホットキー(Ctrl+Alt+S)経由のRustイベントを購読し、
    // VoiceBarのマイクボタン経由と同じstate更新に収束させる(マウント時に1回だけ)。
    let cleanup: (() => void) | undefined;
    useShioriStore
      .getState()
      .subscribeToBackendEvents()
      .then((unsubscribe) => {
        cleanup = unsubscribe;
      });
    return () => cleanup?.();
  }, []);

  useEffect(() => {
    // アプリ起動時にLLM/Embeddingをsidecarとして自動起動する(RAG検索サーバーは
    // Pythonの別プロセスのため対象外。STTは録音開始時にオンデマンドで起動される)。
    // StrictModeの二重実行があっても実際の起動が1回分に抑えられるよう、
    // ストア側のensureBackendServicesStartedを経由する。
    useShioriStore
      .getState()
      .ensureBackendServicesStarted()
      .then((results) => console.log("backend services:", results))
      .catch((err) => console.error("バックエンドサービスの起動に失敗:", err));
  }, []);

  if (isDebugOpen) {
    return <DebugScreen onClose={() => setIsDebugOpen(false)} />;
  }

  return (
    <>
      {showStartup && <StartupScreen onFinished={() => setShowStartup(false)} />}
      <div className="app">
        <TopBar leftCollapsed={leftCollapsed} onToggleLeftCollapsed={toggleLeftCollapsed} />
        <div className="app__body">
          <section className={`app__left${leftCollapsed ? " app__left--collapsed" : ""}`}>
            <ConversationLog />
            <div className="app__left-bottom">
              <ActivityIndicator />
              <CharacterStage />
              <VoiceBar />
            </div>
          </section>
          <section className="app__right">
            <DesktopArea />
            <Dock onOpenDebug={() => setIsDebugOpen(true)} />
          </section>
        </div>
      </div>
    </>
  );
}

export default App;
