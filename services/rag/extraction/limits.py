"""抽出の上限値。素材は信頼できない入力なので、すべての抽出がここを通る。"""

MAX_FILE_BYTES = 50 * 1024 * 1024  # 素材1ファイルの最大サイズ
MAX_PAGES = 300  # PDFのページ数、スライド数、シート数の上限
MAX_CHARS = 1_000_000  # 1素材から取り出す文字数の上限(超えた分は切り捨て)
TIMEOUT_SECONDS = 60  # 抽出プロセスの実行時間の上限
ZIP_MAX_MEMBERS = 5000  # zip(docx/pptx/xlsx)の中のファイル数の上限
ZIP_MAX_MEMBER_BYTES = 50 * 1024 * 1024  # zipの中の1ファイルを展開した最大サイズ
ZIP_MAX_TOTAL_BYTES = 200 * 1024 * 1024  # zipを全部展開した合計の最大サイズ
ZIP_MAX_RATIO = 100  # 圧縮率(展開後/圧縮後)の上限。zip爆弾の検出
WORKER_OUTPUT_MAX_BYTES = 8 * 1024 * 1024  # 抽出プロセスが返す結果の最大サイズ
