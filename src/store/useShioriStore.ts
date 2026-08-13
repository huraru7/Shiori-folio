import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { api, type ServiceStatus } from "../api/tauri";
import type {
  ActiveTool,
  KnowledgeResult,
  OrbStatus,
  ShioriMessage,
} from "../types";

// モジュールスコープで保持し、subscribeToBackendEventsの多重登録を防ぐガード。
// React StrictModeの開発時二重実行では、1回目の登録(非同期)が完了する前に
// 2回目の呼び出しが来ることがあるため、進行中のPromise自体を共有して
// 呼び出しごとに別々のリスナーが登録されないようにする。
let backendEventsSetup: Promise<() => void> | null = null;

// 同様にstartBackendServicesの多重起動を防ぐガード。App.tsxのuseEffectが
// StrictModeで二重実行されると、非同期処理完了前に2回目の呼び出しが来て
// LLM/embeddingのプロセスが2つずつ起動してしまう(VRAMを二重消費する)ため、
// 進行中のPromiseを共有して1回分の起動だけが実行されるようにする。
let backendServicesStartup: Promise<ServiceStatus[]> | null = null;

// start_backend_services自体はRust側の設計上、各サービスのhealthy:falseを
// 含んでいても常にOk(...)で返ってくる(通信エラー等の「本当の例外」とは区別が
// つかない状態でinvoke()が単に成功してしまう)。呼び出し元がhealthyを見落として
// 「サーバー未起動のままホーム画面へ進む」退行(2026-08-13に発覚)を防ぐため、
// いずれかのサービスがhealthy:falseの場合はこのエラーとして投げ、通信エラー等の
// 本当の例外と区別できるようにする。
export class BackendStartupError extends Error {
  constructor(public services: ServiceStatus[]) {
    super("バックエンドサービスの起動に失敗しました");
    this.name = "BackendStartupError";
  }
}

interface ShioriState {
  // 今まさに実行中のツール(表示用の一時的な状態)。ユーザーが手動で選ぶ
  // ものではなく、バックエンドが実行を開始したタイミングでshiori:activity
  // イベント経由で"idle"以外に変わり、応答が返り次第"idle"に戻る。
  activeTool: ActiveTool;
  setActiveTool: (t: ActiveTool) => void;

  orbStatus: OrbStatus;
  setOrbStatus: (s: OrbStatus) => void;

  messages: ShioriMessage[];
  addMessage: (msg: ShioriMessage) => void;

  knowledgeResults: KnowledgeResult[];
  setKnowledgeResults: (r: KnowledgeResult[]) => void;

  // テキスト送信の共通処理。VoiceBarの手入力と、ホットキー/マイクボタン経由の
  // 文字起こし結果の両方がここに収束する(send_messageの呼び出し、応答表示、
  // TTS再生をまとめて行う)。
  processUserText: (text: string) => Promise<void>;

  isRecording: boolean;
  toggleRecording: () => Promise<void>;

  // ホットキー(Rust側)からのイベントを購読し、マイクボタン経由と同じstate更新に
  // 収束させる。App.tsxのマウント時に1回だけ呼び、返り値のクリーンアップ関数を
  // アンマウント時に呼ぶことでリスナーの重複登録を防ぐ。
  subscribeToBackendEvents: () => Promise<() => void>;

  // LLM/Embeddingをsidecarとして起動する。App.tsxのマウント時に1回だけ呼ぶ想定だが、
  // StrictModeの二重実行があっても実際の起動は1回分だけに抑える。
  ensureBackendServicesStarted: () => Promise<ServiceStatus[]>;

  // 起動画面(StartupScreen)向けの「現在何をしているか」の表示テキスト。
  // shiori:startup-stageイベントを購読して更新する(ensureBackendServicesStarted内で
  // 起動呼び出しより先にリスナー登録を済ませることで、登録前に発火したイベントを
  // 取りこぼすレースコンディションを防いでいる)。
  startupStageLabel: string;
}

export const useShioriStore = create<ShioriState>((set, get) => ({
  activeTool: "idle",
  setActiveTool: (t) => set({ activeTool: t }),

  orbStatus: "idle",
  setOrbStatus: (s) => set({ orbStatus: s }),

  messages: [],
  addMessage: (msg) => set({ messages: [...get().messages, msg] }),

  knowledgeResults: [],
  setKnowledgeResults: (r) => set({ knowledgeResults: r }),

  processUserText: async (text) => {
    const trimmed = text.trim();
    if (!trimmed) return;

    get().addMessage({ role: "user", text: trimmed });
    get().setOrbStatus("thinking");
    // 前のターンのアクティビティ表示を引き継がないよう、送信のたびにidleへ戻す
    // (今回のターンで実際にツールが動けば、shiori:activityイベントで上書きされる)。
    get().setActiveTool("idle");

    try {
      // Function Calling方式により、モード指定は不要。単一プロンプト+ツール定義で
      // LLMが自律的に判断する(検索/メモ保存/そのまま会話)。
      const { reply, sources } = await api.sendMessage(trimmed);

      get().addMessage({ role: "assistant", text: reply });
      if (sources) {
        get().setKnowledgeResults(sources);
      }

      get().setOrbStatus("speaking");
      try {
        // TTSは既知の不具合により失敗することがあるが、会話フロー自体は継続させる
        await api.synthesizeSpeech(reply);
      } catch {
        // 失敗は無視する(Rust側でも警告ログのみに留めている)
      }
    } catch (err) {
      get().addMessage({
        role: "assistant",
        text: `エラーが発生しました: ${String(err)}`,
      });
    } finally {
      get().setOrbStatus("idle");
      get().setActiveTool("idle");
    }
  },

  isRecording: false,
  toggleRecording: async () => {
    if (!get().isRecording) {
      await api.startRecording();
      set({ isRecording: true });
      get().setOrbStatus("listening");
    } else {
      set({ isRecording: false });
      const { text } = await api.stopRecordingAndTranscribe();
      await get().processUserText(text);
    }
  },

  subscribeToBackendEvents: () => {
    // 進行中の登録処理があれば同じPromiseを返す(StrictModeの二重実行対策)
    if (!backendEventsSetup) {
      backendEventsSetup = (async () => {
        const unlistenStarted = await listen("voice:recording-started", () => {
          set({ isRecording: true });
          get().setOrbStatus("listening");
        });
        const unlistenTranscribed = await listen<{ text: string }>(
          "voice:transcribed",
          (event) => {
            set({ isRecording: false });
            get().processUserText(event.payload.text);
          },
        );
        // send_message実行中にバックエンドが「今このツールを実行している」ことを
        // 知らせるイベント(ActivityIndicator/OrbCoreの一時表示用)。
        const unlistenActivity = await listen<{ tool: string }>(
          "shiori:activity",
          (event) => {
            const tool = event.payload.tool;
            if (tool === "search_knowledge" || tool === "memo") {
              get().setActiveTool(tool);
            }
          },
        );
        return () => {
          unlistenStarted();
          unlistenTranscribed();
          unlistenActivity();
          backendEventsSetup = null;
        };
      })();
    }
    return backendEventsSetup;
  },

  startupStageLabel: "起動しています",

  ensureBackendServicesStarted: () => {
    if (!backendServicesStartup) {
      backendServicesStartup = (async () => {
        // Rust側は起動処理を開始した直後から段階イベントを発行するため、
        // 先にリスナーを登録してから起動を呼び出す(順序を逆にすると、
        // 登録が完了する前に発行された最初のイベントを取りこぼす)。
        const unlisten = await listen<{ label: string }>(
          "shiori:startup-stage",
          (event) => {
            set({ startupStageLabel: event.payload.label });
          },
        );
        let results: ServiceStatus[];
        try {
          results = await api.startBackendServices();
        } finally {
          unlisten();
        }
        // start_backend_services自体はhealthy:falseでもOk(...)を返す設計のため、
        // ここで明示的にチェックする(このチェックを飛ばすと、RAGサーバーが
        // 起動できていないままホーム画面へ進んでしまう)。
        const unhealthy = results.filter((r) => !r.healthy);
        if (unhealthy.length > 0) {
          // 失敗した状態のPromiseをキャッシュしたままにすると、再試行後に
          // このメソッドを呼び直しても同じ失敗結果が返り続けてしまうため、
          // キャッシュをクリアして次回呼び出し時に再度起動処理が走るようにする。
          backendServicesStartup = null;
          throw new BackendStartupError(results);
        }
        return results;
      })();
    }
    return backendServicesStartup;
  },
}));
