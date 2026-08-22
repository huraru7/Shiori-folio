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
}

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
