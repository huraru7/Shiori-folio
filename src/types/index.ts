// 単一人格+Function Calling+常時バックグラウンド検索(passive recall)方式では、
// モードをユーザーが選ぶ概念は存在しない。ActiveToolは「今まさに実行中の
// ツール」を表す一時的な状態(ActivityIndicator/OrbCoreの表示用)。
// search_knowledge/memo実行中はそのツール名になり、それ以外は"idle"。
// タスク管理機能(update_task)は廃止済み(メモ機能に置き換え、2026-08-08)。
export type ActiveTool = "idle" | "search_knowledge" | "memo";

export type OrbStatus = "idle" | "listening" | "thinking" | "speaking";

export interface ShioriMessage {
  role: "user" | "assistant";
  text: string;
}

export interface KnowledgeResult {
  id: string;
  text: string;
  source: string;
  heading: string;
  sourceCategory: string;
}

// スタンドアロン図書館UI(Phase 7、1冊=1ファイルの表示単位)向け。チャンクの
// 本文は含まず、ファイル内の見出し一覧だけを持つ。
export interface LibraryFile {
  source: string;
  sourceCategory: string;
  headings: string[];
  // frontmatterのtitle(無ければ空文字列)・実ファイルへの絶対パス
  // (2026-09-16追加、エクスプローラー風UI刷新向け)。
  title: string;
  path: string;
  // .meta(素材のサイドカー)のとき対応する実体の絶対パス。.metaでない・実体が
  // 無い(孤児)場合は空文字列(詩織Ver3.5)。
  assetPath: string;
  // library_rootからの相対パス(/区切り)。フォルダツリー構築に使う。
  relativePath: string;
  // ファイルの更新日時(Unixタイムスタンプ、秒)。
  mtime: number;
}

// ライブラリウィンドウの検索結果ベースUI(Ver3.0、UI改善4-2節)向け。
// search_libraryはファイル単位に集約された、ヒットした見出しとスコアの
// リストを返す(本文は含まない)。
export interface SearchLibraryHeading {
  heading: string;
  rerankScore: number;
}

export interface SearchLibraryResult {
  source: string;
  sourceCategory: string;
  headings: SearchLibraryHeading[];
  bestScore: number;
  // 実ファイルへの絶対パス・frontmatterのtitle(2026-09-16追加)。
  path: string;
  // .metaのとき対応する実体の絶対パス(無ければ空文字列、詩織Ver3.5)。
  assetPath: string;
  title: string;
  // 一覧に添える属性と冒頭の文(詩織Ver3.9)。記事を読めなかったときは空文字列。
  summary: string;
  project: string;
  entryKind: string;
  status: string;
  date: string;
}

// 図書館の検索の絞り込み(詩織Ver3.9)。空の項目は絞り込まない。
export interface LibraryFilter {
  // falseなら、アーカイブ(90-archive)とstatusがoutdated/deprecatedの記録を除く。
  includeStale: boolean;
  sourceCategories: string[];
  project: string | null;
  kind: string | null;
}

// ホームの「前回の申し送り」(詩織Ver3.9)。
export interface Handoff {
  title: string;
  project: string;
  date: string;
  source: string;
  sourceCategory: string;
  items: string[];
}

// 記事詳細画面(Ver3.0、UI改善4-2節)向け。frontmatterの構造化フィールド。
export interface SourceFrontmatter {
  title: string | null;
  type: string | null;
  tags: string[];
  project: string | null;
  summary: string | null;
  index: boolean;
  status: string;
  related: string[];
}

// 要確認UI(Phase 8)向け。tags.yaml/projects.yamlのstatus、inboxファイルの
// review_statusはいずれも"pending"(未着手)/"deferred"(保留中)のいずれか
// (confirmed済みのものはバックエンド側で一覧から除外されている)。
export type PendingStatus = "pending" | "deferred";

export interface PendingTag {
  canonical: string;
  status: PendingStatus;
}

export interface PendingProject {
  id: string;
  status: PendingStatus;
}

export interface PendingInboxItem {
  filename: string;
  title: string | null;
  reason: string | null;
  reviewStatus: PendingStatus | null;
}

export interface PendingItems {
  tags: PendingTag[];
  projects: PendingProject[];
  inbox: PendingInboxItem[];
}

// resolvePendingTag/resolvePendingProject/resolvePendingInboxItem共通のaction。
// confirm=承認、reject=却下(エントリ/ファイル削除)、defer=保留。
export type PendingAction = "confirm" | "reject" | "defer";

export type TagVariant = "warm" | "teal" | "cyan";

export interface GpuInfo {
  name: string;
  vramUsedMb: number;
  vramTotalMb: number;
  temperatureC: number | null;
}

export interface ServiceInfo {
  name: string;
  running: boolean;
  vramMb: number | null;
  ramMb: number | null;
  modelName: string | null;
}

export interface DiskThroughput {
  readMbPerSec: number;
  writeMbPerSec: number;
}

export interface StorageInfo {
  drive: string;
  kind: string;
  fileSystem: string;
  totalGb: number;
  freeGb: number;
  isRemovable: boolean;
}

export interface PowerInfo {
  onBattery: boolean;
  batteryPercent: number | null;
}

export interface CpuDetail {
  model: string;
  physicalCores: number;
  logicalCores: number;
  frequencyMhz: number;
}

export interface SystemInfo {
  gpu: GpuInfo | null;
  ramUsedMb: number;
  ramTotalMb: number;
  cpuUsagePercent: number;
  os: string;
  // "windows" / "macos" / "linux"。VRAM表示可否等のOS判定に使う。
  platform: string;
  cpu: CpuDetail;
  services: ServiceInfo[];
  diskThroughput: DiskThroughput | null;
  storage: StorageInfo | null;
  power: PowerInfo | null;
}

export interface AppConfigDto {
  hotkey: string;
  llmPort: number;
  llmContextSize: number | null;
  embeddingPort: number;
  sttPort: number;
  ragPort: number;
  passiveRecallThreshold: number;
  lengthScale: number;
  noiseScale: number;
  noiseW: number;
  showYear: boolean;
  showMonth: boolean;
  showDay: boolean;
  showWeekday: boolean;
  showSeconds: boolean;
  mode: AppMode;
}

// 会話モード/外部AIモード(詩織Ver4.0)。
export type AppMode = "conversation" | "external";

// get_config()の全項目をそのままOptionalにした部分更新用の型。
export type AppConfigUpdate = Partial<AppConfigDto>;

export interface ModelInfo {
  fileName: string;
  sizeMb: number;
  vramEstimateGb: number;
  isMeasured: boolean;
  isCurrent: boolean;
}

export interface ModelSwitchEstimate {
  projectedVramPercent: number;
  isRisky: boolean;
}

export interface ModelSwitchResult {
  success: boolean;
  measuredVramGb: number | null;
  rolledBack: boolean;
  error: string | null;
}

export interface TtsFailure {
  error: string;
  createdAt: string;
}

export interface PassiveRecallStats {
  hits: number;
  total: number;
}

export interface RagasHistory {
  headers: string[];
  rows: string[][];
}

// Claudeモニター(Ver3.7)。state: working=作業中 / waiting=入力待ち / idle=待機 /
// ended=終了 / stale=更新が止まったまま(応答なし)。時刻はエポックミリ秒。
export type ClaudeSessionState = "working" | "waiting" | "idle" | "ended" | "stale";

export interface ClaudeSession {
  sessionId: string;
  project: string;
  title: string;
  host: string;
  state: ClaudeSessionState;
  startedAt: number;
  updatedAt: number;
}

// Claudeの利用統計(詩織Ver3.9)。「実際」は応答・発言のIDで重複を除いた値、
// 「アプリ式」はデスクトップアプリの統計パネルと同じく履歴の行ごとに数えた値。
export type ClaudeStatsRange = "all" | "days30" | "days7";
export type ClaudeStatsCounting = "actual" | "app";

export interface ClaudeTokens {
  input: number;
  cacheCreation: number;
  cacheRead: number;
  output: number;
}

export interface ClaudeStats {
  hosts: { host: string; updatedAt: number }[];
  since: string;
  until: string;
  sessions: number;
  messages: number;
  messagesApp: number;
  activeDays: number;
  actual: ClaudeTokens;
  app: ClaudeTokens;
  daily: { date: string; actual: number; app: number }[];
  models: { model: string; actual: number; app: number }[];
  warnings: string[];
}

// 取り外し機能。止める予定のもの(toStop)。
export interface EjectEntry {
  pid: number;
  parentPid: number | null;
  name: string;
  exe: string;
  // 止めたときの影響の説明(mcp_serverを使っているClaudeのセッション等)。
  note: string;
}

export interface EjectPlan {
  toStop: EjectEntry[];
}

export interface EjectPreview {
  plan: EjectPlan;
  // 止める予定のうち、詩織を閉じても残るもの(MCPなどが起動した共有デーモン)。
  leftoverAfterExit: EjectEntry[];
  // lsofで見つかった、ほかにSSDを開いているプロセス [pid, 名前]。
  otherHolders: [number, string][];
}

export interface EjectOutcome {
  stopped: number[];
  forced: number[];
  failed: number[];
}
