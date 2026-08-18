#!/usr/bin/env bash
# ローカルで生成済みのportable/(build-portable.shの出力)を、USB等の別ドライブ上の
# 既存portable/へ安全に反映するスクリプト(Mac/Linux)。
# 配置場所: system/scripts/deploy-portable-to.sh
# 使用法: bash scripts/deploy-portable-to.sh /Volumes/<ドライブ名>/portable
#
# bin/<os>/(アプリ本体・エンジン群・RAG用Python環境)とservices/rag/(ソース)
# のみを反映し、config.json/prompts/models/data/library/には一切触れない
# (USB側の実データ・設定を壊さないため)。
#
# 【2026-08-18修正】USB(exFAT等、Unix拡張属性を持たないファイルシステム)へ
# 素の`rsync -a`でコピーすると、macOSがxattr保持のため大量の`._*`
# (AppleDouble)ファイルを生成し、その一部がPythonパッケージ(transformers等)の
# モジュール探索に混入してRAGサーバーがクラッシュする不具合が実機で発生した
# (USB版Ver2.0更新作業で発覚・2回再発)。COPYFILE_DISABLE=1でmacOS側の
# AppleDouble生成自体を止めた上で、念のため同期後に取りこぼしを掃除する。
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "使用法: $0 <展開先のportable/パス>" >&2
  echo "例: $0 /Volumes/ShioriFolio/portable" >&2
  exit 1
fi

TARGET_DIR="$1"
if [ ! -d "$TARGET_DIR" ]; then
  echo "エラー: 展開先が見つかりません: $TARGET_DIR" >&2
  echo "(既存のportable/へ反映する想定のため、事前に作成しておくこと)" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SYSTEM_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$SYSTEM_DIR/.." && pwd)"
LOCAL_PORTABLE="$REPO_ROOT/portable"

case "$(uname -s)" in
  Darwin) TARGET_OS="mac" ;;
  Linux) TARGET_OS="linux" ;;
  *) echo "未対応のOSです: $(uname -s)" >&2; exit 1 ;;
esac

if [ ! -d "$LOCAL_PORTABLE/bin/$TARGET_OS" ]; then
  echo "エラー: $LOCAL_PORTABLE/bin/$TARGET_OS が見つかりません。先に build-portable.sh を実行してください。" >&2
  exit 1
fi

echo "=== $TARGET_DIR へ反映開始 (OS: $TARGET_OS) ==="

# AppleDouble(._*)ファイルの生成自体を止める(上記コメント参照)。
export COPYFILE_DISABLE=1

echo "--- bin/$TARGET_OS/ ---"
rsync -a --delete "$LOCAL_PORTABLE/bin/$TARGET_OS/" "$TARGET_DIR/bin/$TARGET_OS/"

echo "--- services/rag/ (ソースのみ) ---"
mkdir -p "$TARGET_DIR/services/rag"
rsync -a --delete \
  --exclude ".venv" \
  --exclude "__pycache__" \
  --exclude "eval" \
  "$LOCAL_PORTABLE/services/rag/" "$TARGET_DIR/services/rag/"

# COPYFILE_DISABLEでほぼ発生しないはずだが、念のため取りこぼしを掃除する
# (find -deleteは大量件数だと1回で消しきれないことがあるため、0件になるまで
# 繰り返す。2026-08-18の実機確認で複数回必要なケースを確認済み)。
echo "--- AppleDoubleファイルの掃除(念のため) ---"
while [ "$(find "$TARGET_DIR" -name "._*" 2>/dev/null | wc -l | tr -d ' ')" != "0" ]; do
  find "$TARGET_DIR" -name "._*" -delete 2>/dev/null || true
done

echo "=== 反映完了 ==="
