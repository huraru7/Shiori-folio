// 図書館UI共通のヘルパー。「本」の見た目(色分け・分類番号)を、chunkの
// メタデータから機械的に導出する。KnowledgePanel(3-1)・ChunkModal(3-3)・
// LibraryScreen(3-2)で共有する。

export interface CategoryMeta {
  abbr: string;
  label: string;
  // src/styles/tokens.cssの--category-*と同じ値。CSSクラスで足りない箇所
  // (バッジの半透明背景等)をインラインstyleで組み立てるために使う。
  hex: string;
}

// 3-5節の色分けルール。配色は2026-08-12のUI/UX改善(苔色/テラコッタ基調への
// 全面転換)に合わせてtokens.cssの--category-*と同じ値にしている。
//
// 【2026-08-22更新】Ver3.0(データ管理法見直し2-2節)でlibrary/のディレクトリ
// 体系が旧(00-inbox〜90-archive、8フォルダ)から新(10-projects/20-areas/
// 30-resources/40-journal、PARAベース)へ再編された。ここを旧カテゴリのまま
// 放置すると、Phase 7移行時(2026-08-17)と同じ「未分類」表示バグを繰り返す
// ため、フォルダ再編と同じタイミングで更新する。新カテゴリの配色は旧カテゴリの
// 色をそのまま引き継いだ(統合先の性質が近いものを機械的に対応させただけで、
// 配色自体の作り込みはUI刷新〈Phase 5〉で改めて検討する)。
const CATEGORY_META: Record<string, CategoryMeta> = {
  "00-inbox": { abbr: "IB", label: "インボックス", hex: "#8C8577" },
  "10-projects": { abbr: "PJ", label: "プロジェクト", hex: "#C4914E" },
  "20-areas": { abbr: "AE", label: "エリア", hex: "#6E8FA8" },
  "30-resources": { abbr: "RS", label: "リソース", hex: "#C68A5E" },
  "40-journal": { abbr: "JN", label: "ジャーナル", hex: "#A67B8F" },
  "90-archive": { abbr: "AC", label: "アーカイブ", hex: "#7A7568" },
};

const FALLBACK_CATEGORY_META: CategoryMeta = {
  abbr: "UC",
  label: "未分類",
  hex: "#A79E8C",
};

export function getCategoryMeta(sourceCategory: string): CategoryMeta {
  return CATEGORY_META[sourceCategory] ?? FALLBACK_CATEGORY_META;
}

// スタンドアロン図書館(3-2)の棚を並べる順序。未知のカテゴリ(uncategorized等)は
// 末尾にまとめる。ディレクトリの番号プレフィックス順(library/_system/CLAUDE.md
// の配置ルール1〜4の並びと同じ)。
export const CATEGORY_ORDER = [
  "00-inbox",
  "10-projects",
  "20-areas",
  "30-resources",
  "40-journal",
  "90-archive",
] as const;

// 16進カラーコードをrgba()文字列に変換する(バッジの半透明背景・枠線用)。
export function hexToRgba(hex: string, alpha: number): string {
  const clean = hex.replace("#", "");
  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

// Book.cssの`.book--{category}`セレクタに対応するクラス名。未知のカテゴリは
// `book--uncategorized`にフォールバックする。
export function getBookCategoryClass(sourceCategory: string): string {
  return `book--${sourceCategory in CATEGORY_META ? sourceCategory : "uncategorized"}`;
}

// チャンクIDは`{category}-{stem}-{i}`形式(services/rag/ingest.py)。実装方針①
// (2026-08-12)により、末尾の連番をそのままcallnoの番号部分に流用する
// (一覧の並び順で振り直すとデータの増減のたびに番号がズレるため)。
export function getCallNo(chunkId: string, sourceCategory: string): string {
  const { abbr } = getCategoryMeta(sourceCategory);
  const lastDash = chunkId.lastIndexOf("-");
  const tail = lastDash >= 0 ? chunkId.slice(lastDash + 1) : "";
  const num = /^\d+$/.test(tail) ? tail.padStart(2, "0") : "??";
  return `${abbr}-${num}`;
}

// Phase 7(1冊=1ファイルの表示単位)向け。チャンクIDを持たないファイル単位の
// 一覧のため、カテゴリ棚内での並び順(index)をそのまま番号として使う。
export function getFileCallNo(index: number, sourceCategory: string): string {
  const { abbr } = getCategoryMeta(sourceCategory);
  return `${abbr}-${String(index + 1).padStart(2, "0")}`;
}

// ファイルの表紙タイトル。先頭の実見出し(chunking.pyの「(見出しなし)」
// プレースホルダーを除く)があればそれを使い、無ければファイル名を使う。
export function getFileTitle(source: string, headings: string[]): string {
  const firstHeading = headings.find((h) => h && h !== "(見出しなし)");
  return firstHeading ?? source.replace(/\.md$/i, "");
}

// 「司書が持ってくる棚」(3-1)の集まってくる演出で、本ごとに少しずつ違う方向
// から飛んでくるように見せるためのオフセットプリセット(library-ui-concept.html
// のモックアップの値をそのまま移植)。本の数がプリセット数を超えたら周回する。
const FLY_FROM_PRESETS: { fx: string; fy: string; fr: string }[] = [
  { fx: "-140px", fy: "-40px", fr: "-18deg" },
  { fx: "120px", fy: "-70px", fr: "14deg" },
  { fx: "-90px", fy: "60px", fr: "22deg" },
  { fx: "160px", fy: "30px", fr: "-10deg" },
  { fx: "-30px", fy: "-90px", fr: "8deg" },
];

export function getFlyFromOffset(index: number): { fx: string; fy: string; fr: string } {
  return FLY_FROM_PRESETS[index % FLY_FROM_PRESETS.length];
}
