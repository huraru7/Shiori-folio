import { useEffect, useState } from "react";
import TopBar from "./components/TopBar";
import Sidebar from "./components/sidebar/Sidebar";
import MainArea from "./components/main/MainArea";
import PendingModal from "./components/main/PendingModal";
import StartupScreen from "./components/startup/StartupScreen";
import CloseGuard from "./components/eject/CloseGuard";
import ApprovalModal from "./components/extensions/ApprovalModal";
import Effects from "./components/effects/Effects";
import Notices from "./components/notices/Notices";
import { useAppModeStore } from "./store/useAppModeStore";
import { useShioriStore } from "./store/useShioriStore";
import { usePendingStore } from "./store/usePendingStore";
import { useSidebarStore } from "./store/useSidebarStore";
import { useExtensionsStore } from "./store/useExtensionsStore";
import { useMotionStore } from "./store/useMotionStore";
import { getSelfTestMode, runSelfTest } from "./lib/selfTest";
import { listen } from "@tauri-apps/api/event";
import { openSource } from "./components/notices/Notices";
import type { Notice } from "./types";
import "./App.css";

// 全体レイアウト(詩織Ver6.0の段階2)。上の帯(全幅)の下に、左の縦の列と、選んだ画面を1つ出す
// 場所(MainArea)を並べる。会話も列の画面の1つ。それまでの「左に会話・右に上のタブ」の形は、
// 増える機能に耐えないため、構想メモで左の縦の列に決めた。
// Zustandはグローバルフックのため、Providerは不要。
function App() {
  const [showStartup, setShowStartup] = useState(true);
  const pendingOpen = usePendingStore((s) => s.open);

  useEffect(() => {
    useAppModeStore.getState().load();
    usePendingStore.getState().load();
    useSidebarStore.getState().load();
    useExtensionsStore.getState().load();
    useMotionStore.getState().load();
  }, []);

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

  useEffect(() => {
    // ペットの吹き出しから、ウィンドウに戻ってその知らせの元を開く(詩織Ver6.0の段階4)
    const stop = listen<Notice>("shiori://open-source", (e) => openSource(e.payload));
    return () => {
      stop.then((u) => u());
    };
  }, []);

  useEffect(() => {
    // 自己確認の起動(詩織Ver6.0の段階2)なら、起動画面が消えてから列の項目を全部確かめる
    if (showStartup) return;
    getSelfTestMode().then((m) => {
      if (m.enabled) runSelfTest();
    });
  }, [showStartup]);

  return (
    <>
      <CloseGuard />
      <Effects />
      {showStartup && <StartupScreen onFinished={() => setShowStartup(false)} />}
      {/* 拡張機能の承認(詩織Ver6.0の段階1)。起動画面が消えてから出す */}
      {!showStartup && <ApprovalModal />}
      <div className="app">
        <TopBar />
        <div className="app__body">
          <Sidebar />
          <section className="app__main">
            <MainArea />
          </section>
        </div>
      </div>
      {/* 知らせの札(詩織Ver6.0の段階3) */}
      <Notices />
      {pendingOpen && (
        <PendingModal
          onClose={() => usePendingStore.getState().setOpen(false)}
          onItemsChange={(items) => usePendingStore.getState().setItems(items)}
        />
      )}
    </>
  );
}

export default App;
