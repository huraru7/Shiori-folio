import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfigDto,
  AppConfigUpdate,
  KnowledgeResult,
  ModelInfo,
  ModelSwitchEstimate,
  ModelSwitchResult,
  PassiveRecallStats,
  RagasHistory,
  SystemInfo,
  TtsFailure,
} from "../types";

export interface ServiceStatus {
  name: string;
  port: number;
  started: boolean;
  healthy: boolean;
  error?: string;
}

// 仕様書v2.0「9. Tauriコマンド」で定義した各コマンドの型付きラッパー。
// Tauriのinvoke()を直接importするのはこのファイルだけに限定する。
export const api = {
  startBackendServices: () =>
    invoke<ServiceStatus[]>("start_backend_services"),


  // Function Calling方式(案②)により、モード指定は不要。単一プロンプト+ツール定義で
  // LLMが自律的に判断する。detectedModeは結果として「どのツールが呼ばれたか」を表す。
  sendMessage: (text: string) =>
    invoke<{
      reply: string;
      sources?: KnowledgeResult[];
      detectedMode: string;
    }>("send_message", { text }),

  startRecording: () => invoke<void>("start_recording"),

  stopRecordingAndTranscribe: () =>
    invoke<{ text: string }>("stop_recording_and_transcribe"),

  synthesizeSpeech: (text: string) =>
    invoke<void>("synthesize_speech", { text }),

  // コントロールパネルの「システム状態」向け。1〜2秒間隔でポーリングされる想定。
  getSystemInfo: () => invoke<SystemInfo>("get_system_info"),

  // コントロールパネルの「音声設定」試し読み用。config.jsonへの保存はしない。
  previewVoice: (text: string, lengthScale: number, noiseScale: number, noiseW: number) =>
    invoke<void>("preview_voice", {
      text,
      lengthScale,
      noiseScale,
      noiseW,
    }),

  getConfig: () => invoke<AppConfigDto>("get_config"),

  setConfig: (update: AppConfigUpdate) => invoke<void>("set_config", { update }),

  restartLlmServices: () => invoke<ServiceStatus[]>("restart_llm_services"),

  restartApp: () => invoke<void>("restart_app"),

  listAvailableModels: () => invoke<ModelInfo[]>("list_available_models"),

  estimateModelSwitch: (fileName: string) =>
    invoke<ModelSwitchEstimate>("estimate_model_switch", { fileName }),

  switchModel: (fileName: string) => invoke<ModelSwitchResult>("switch_model", { fileName }),

  // KnowledgePanelの参照情報クリック時、元のMarkdown全文を取得する(v1.0機能1)。
  getSourceDocument: (sourceCategory: string, source: string) =>
    invoke<string>("get_source_document", { sourceCategory, source }),

  // スタンドアロン図書館UI向け、検索を経由しない蔵書全件の一覧取得
  // (2026-08-12、図書館ビジョン統合仕様書3-2)。
  listAllKnowledge: () => invoke<KnowledgeResult[]>("list_all_knowledge"),

  // 以下3つはコントロールパネルの「デバッグ」画面向け(v1.0機能2)。
  getTtsFailures: () => invoke<TtsFailure[]>("get_tts_failures"),

  getPassiveRecallStats: () => invoke<PassiveRecallStats>("get_passive_recall_stats"),

  getRagasHistory: () => invoke<RagasHistory>("get_ragas_history"),
};
