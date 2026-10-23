//! 抽出の上限値(`services/rag/extraction/limits.py` と同じ値)。素材は信頼できない入力なので、
//! すべての抽出がここを通る。

pub const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024; // 素材1ファイルの最大サイズ
pub const MAX_PAGES: usize = 300; // PDFのページ数、スライド数、シート数の上限
pub const MAX_CHARS: usize = 1_000_000; // 1素材から取り出す文字数の上限(超えた分は切り捨て)
pub const TIMEOUT_SECONDS: u64 = 60; // 抽出プロセスの実行時間の上限
pub const ZIP_MAX_MEMBERS: usize = 5000; // zip(docx/pptx/xlsx)の中のファイル数の上限
pub const ZIP_MAX_MEMBER_BYTES: u64 = 50 * 1024 * 1024; // zipの中の1ファイルを展開した最大サイズ
pub const ZIP_MAX_TOTAL_BYTES: u64 = 200 * 1024 * 1024; // zipを全部展開した合計の最大サイズ
pub const ZIP_MAX_RATIO: f64 = 100.0; // 圧縮率(展開後/圧縮後)の上限。zip爆弾の検出
pub const WORKER_OUTPUT_MAX_BYTES: u64 = 8 * 1024 * 1024; // 抽出プロセスが返す結果の最大サイズ
