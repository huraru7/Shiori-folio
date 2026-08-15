#!/usr/bin/env bash
# 段階G: portable/ 運用パッケージ生成スクリプト(Mac/Linux)。
# 配置場所: system/scripts/build-portable.sh
#
# Shiori-folio/直下(system/・library/と並ぶ場所、このスクリプトから見て
# 2階層上)にportable/を生成する。複数OSで同じportable/を共有する想定のため、
# このスクリプトは「自分のOS分のbin/<os>/だけを生成・上書きし、共有リソース
# (prompts/config/models/data/library/services/rag)は毎回作り直す」という
# 設計にしている。Windows側でも同じportable/(同じ場所、例えばUSB上)を
# 指してsystem/scripts/build-portable.ps1を実行すれば、bin/win/が追加される
# 形でマージされる。
#
# このスクリプト自身の場所を基準に相対パスで解決する(絶対パスを焼き込まない、
# 段階Cのproject_root()と同じ教訓)。system/scripts/から移動する場合は、
# 下記SCRIPT_DIR基準の相対階層(SYSTEM_DIR=1つ上、REPO_ROOT=2つ上)も
# 合わせて調整すること。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SYSTEM_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$SYSTEM_DIR/.." && pwd)"
LIBRARY_DIR="$REPO_ROOT/library"
PORTABLE_DIR="$REPO_ROOT/portable"

case "$(uname -s)" in
  Darwin) TARGET_OS="mac" ;;
  Linux) TARGET_OS="linux" ;;
  *) echo "未対応のOSです: $(uname -s)" >&2; exit 1 ;;
esac

echo "=== portable/ 生成開始 (OS: $TARGET_OS) ==="
echo "出力先: $PORTABLE_DIR"

if ! command -v uv >/dev/null 2>&1; then
  echo "エラー: uv が見つかりません(RAG用ポータブルPython環境の構築に使用します)。" >&2
  echo "  brew install uv でインストールしてください。" >&2
  exit 1
fi

mkdir -p "$PORTABLE_DIR"

# --- 1. 共有リソース(OSを問わず同じ内容。実行のたびに作り直す) ---

echo "--- prompts/ ---"
mkdir -p "$PORTABLE_DIR/prompts"
rsync -a --delete "$SYSTEM_DIR/prompts/" "$PORTABLE_DIR/prompts/"

echo "--- config.json ---"
cp "$SYSTEM_DIR/config.json" "$PORTABLE_DIR/config.json"

echo "--- models/ ---"
mkdir -p "$PORTABLE_DIR/models"
rsync -a "$SYSTEM_DIR/models/" "$PORTABLE_DIR/models/"

echo "--- data/vectordb, data/shiori.db ---"
mkdir -p "$PORTABLE_DIR/data"
rsync -a --delete "$SYSTEM_DIR/data/vectordb/" "$PORTABLE_DIR/data/vectordb/"
cp "$SYSTEM_DIR/data/shiori.db" "$PORTABLE_DIR/data/shiori.db"

echo "--- library/ (system/の外、実データをコピー) ---"
mkdir -p "$PORTABLE_DIR/library"
rsync -a --delete "$LIBRARY_DIR/" "$PORTABLE_DIR/library/"

echo "--- services/rag/ (ソースのみ。.venv/__pycache__/evalは除外) ---"
mkdir -p "$PORTABLE_DIR/services/rag"
rsync -a --delete \
  --exclude ".venv" \
  --exclude "__pycache__" \
  --exclude "eval" \
  "$SYSTEM_DIR/services/rag/" "$PORTABLE_DIR/services/rag/"

# --- 2. OS別バイナリ配置(bin/<os>/) ---

BIN_DIR="$PORTABLE_DIR/bin/$TARGET_OS"
mkdir -p "$BIN_DIR"

if [ "$TARGET_OS" = "mac" ]; then
  APP_SRC="$SYSTEM_DIR/src-tauri/target/release/bundle/macos/shiori-folio.app"
  if [ ! -d "$APP_SRC" ]; then
    echo "エラー: $APP_SRC が見つかりません。先に (cd system && npx tauri build) を実行してください。" >&2
    exit 1
  fi
  echo "--- shiori-folio.app ---"
  rm -rf "$BIN_DIR/shiori-folio.app"
  cp -R "$APP_SRC" "$BIN_DIR/shiori-folio.app"

  # llama-server/whisper-serverは@rpath経由で複数のdylib(libggml*・libllama*・
  # libwhisper*)と、Homebrewの絶対パスにあるlibssl/libcrypto(llama-serverのみ)に
  # 依存しており、バイナリ単体をコピーしただけでは起動できない
  # (2026-08-13、実機確認で発覚)。dylibbundlerで依存dylibを一緒にコピーし、
  # 参照パスを@executable_path相対に書き換えて自己完結させる。
  #
  # 2エンジンは同名のdylib(libggml-base.dylib等)を持つが、それぞれ別ビルドで
  # 中身が異なる(実機でMD5不一致を確認済み)。1つのlibs/フォルダにまとめると
  # 上書き事故で片方が壊れるため、libs-llama/・libs-whisper/と分けて配置する。
  # dylibbundlerは処理対象ディレクトリ内を再帰的に走査するため、rag-venv等の
  # 巨大なディレクトリと同じ場所で実行すると極端に遅くなる(実機で24分以上
  # 応答なしを確認)。作業用の隔離ディレクトリで処理してから最終配置へコピーする。
  echo "--- エンジンバイナリ(llama-server / whisper-server / piper-plus-cli) ---"
  if ! command -v dylibbundler >/dev/null 2>&1; then
    echo "エラー: dylibbundler が見つかりません。brew install dylibbundler でインストールしてください。" >&2
    exit 1
  fi

  WORK_DIR="$(mktemp -d)"
  trap 'rm -rf "$WORK_DIR"' EXIT

  mkdir -p "$WORK_DIR/llama"
  cp "$SYSTEM_DIR/third_party/llama.cpp/build/bin/llama-server" "$WORK_DIR/llama/"
  cp "$SYSTEM_DIR/third_party/llama.cpp/build/bin/"*.dylib "$WORK_DIR/llama/"
  (cd "$WORK_DIR/llama" && dylibbundler -od -b -x ./llama-server -d ./libs-llama -p "@executable_path/libs-llama/" >/dev/null)
  cp "$WORK_DIR/llama/llama-server" "$BIN_DIR/llama-server"
  rm -rf "$BIN_DIR/libs-llama"
  cp -R "$WORK_DIR/llama/libs-llama" "$BIN_DIR/libs-llama"

  mkdir -p "$WORK_DIR/whisper"
  cp "$SYSTEM_DIR/third_party/whisper.cpp/build/bin/whisper-server" "$WORK_DIR/whisper/"
  cp "$SYSTEM_DIR/third_party/whisper.cpp/build/bin/"*.dylib "$WORK_DIR/whisper/"
  (cd "$WORK_DIR/whisper" && dylibbundler -od -b -x ./whisper-server -d ./libs-whisper -p "@executable_path/libs-whisper/" >/dev/null)
  cp "$WORK_DIR/whisper/whisper-server" "$BIN_DIR/whisper-server"
  rm -rf "$BIN_DIR/libs-whisper"
  cp -R "$WORK_DIR/whisper/libs-whisper" "$BIN_DIR/libs-whisper"

  rm -rf "$WORK_DIR"
  trap - EXIT

  # piper-plus-cliは静的リンク中心でOSフレームワーク以外への依存が無いことを
  # 実機確認済み(otool -Lで@rpath依存なし)、単体コピーで問題ない。
  cp "$SYSTEM_DIR/third_party/piper-plus/src/rust/target/release/piper-plus-cli" "$BIN_DIR/piper-plus-cli"

  # shiori-save(Ver2.0 Phase 5、保存CLI)もOSフレームワークのみに依存
  # (otool -Lで確認済み、@rpath依存なし)のため単体コピーで問題ない。
  # cargo buildの成果物名はshiori_save(アンダースコア)だが、CLIとしての
  # 呼び出し名(shiori-save)に合わせてコピー時にリネームする。
  SHIORI_SAVE_SRC="$SYSTEM_DIR/src-tauri/target/release/shiori_save"
  if [ ! -f "$SHIORI_SAVE_SRC" ]; then
    echo "エラー: $SHIORI_SAVE_SRC が見つかりません。先に (cd system && npx tauri build) を実行してください。" >&2
    exit 1
  fi
  cp "$SHIORI_SAVE_SRC" "$BIN_DIR/shiori-save"

  chmod +x "$BIN_DIR/llama-server" "$BIN_DIR/whisper-server" "$BIN_DIR/piper-plus-cli" "$BIN_DIR/shiori-save"
else
  echo "警告: $TARGET_OS 向けのバイナリ配置は未対応のためスキップしました。" >&2
fi

# --- 3. RAG用の可搬版Python環境(bin/<os>/rag-venv/) ---
#
# uv venvはbin/pythonが移動元への絶対パスシンボリックリンクになり、USBの
# 別ドライブ文字や別フォルダへ移動すると壊れることを確認した(2026-08-13)。
# そのため、venvを介さず「uv python installで取得したポータブルPython本体に
# 直接パッケージをインストールする」方式にしている(移動後も動作することを
# 実機確認済み)。requirements.txtのうちragas(評価専用スクリプトeval/でのみ
# 使用、サーバー本体は不使用)は意図的に除外し、サイズを抑えている。

echo "--- RAG用ポータブルPython環境(bin/$TARGET_OS/rag-venv/) ---"
RAG_VENV="$BIN_DIR/rag-venv"
rm -rf "$RAG_VENV"
mkdir -p "$RAG_VENV.tmp"
uv python install -i "$RAG_VENV.tmp" 3.12
PY_INSTALLED_DIR=$(find "$RAG_VENV.tmp" -maxdepth 1 -type d -name "cpython-*")
mv "$PY_INSTALLED_DIR" "$RAG_VENV"
rm -rf "$RAG_VENV.tmp"
ln -sf python3.12 "$RAG_VENV/bin/python"

uv pip install --python "$RAG_VENV/bin/python" --break-system-packages \
  chromadb fastapi "uvicorn[standard]" httpx tiktoken sentence-transformers

# --- 4. ランチャー ---

if [ "$TARGET_OS" = "mac" ]; then
  echo "--- ランチャー(詩織を起動.command) ---"
  cat > "$PORTABLE_DIR/詩織を起動.command" <<'LAUNCHER'
#!/usr/bin/env bash
# 詩織(Shiori-folio) ポータブル版ランチャー(Mac)。
# このファイル自身の場所を基準に相対パスで解決するため、USBのどこに
# 置いても、どのドライブ文字/マウントパスでも動作する(絶対パスを
# 焼き込まない。段階Cのproject_root()修正と同じ考え方)。
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
open "$DIR/bin/mac/shiori-folio.app"
LAUNCHER
  chmod +x "$PORTABLE_DIR/詩織を起動.command"
fi

echo "=== portable/ 生成完了 ==="
du -sh "$PORTABLE_DIR"
