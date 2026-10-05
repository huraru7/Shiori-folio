"""knowledge_dir(library/)の解決。app.py/ingest.pyで共通利用する。

開発ツリーではsystem/とlibrary/が兄弟ディレクトリだが、ポータブル版は
portable/直下にlibrary/を持つ(portable/services/rag/から見ると兄弟ではなく
2階層上の子)ため、単純に「PROJECT_ROOTの親のlibrary/」を見るだけでは
ポータブル版で誤ったパス(存在しない場所)を指してしまう
(2026-08-15、USB版Ver2.0移行の実機確認で発覚)。

Rust側のlibrary_root()(src-tauri/src/lib.rs)と同じ判定にしている:
PROJECT_ROOT直下にlibrary/があれば(ポータブル版)そちらを優先し、
無ければ(開発ツリー)従来通り親ディレクトリのlibrary/を使う。
"""
from __future__ import annotations

import os
from pathlib import Path


def resolve_knowledge_dir(project_root: Path) -> Path:
    portable_library = project_root / "library"
    if portable_library.is_dir():
        return portable_library
    return project_root.parent / "library"


# ChromaDBのRust版SQLiteバインディング(chromadb_rust_bindings)が、外部SSD
# (ExFAT、macOSの新しいfskit経由マウント)上で新規DB作成の最初の接続から
# 「attempt to write a readonly database」で失敗する不具合が実機で見つかった
# (2026-10-02)。Python標準のsqlite3モジュール(WALモード含む)では同じボリューム
# で問題なく書き込めることを確認済みで、ファイルシステムそのものではなく
# ChromaDB側のSQLite実装が使うロック機構とfskitの組み合わせ固有の非互換と見られる。
# vectordbは検索インデックス(library/から再構築可能な派生データ)なので、
# 環境変数SHIORI_VECTORDB_DIRが設定されていればそちらを優先して使い、外部SSDの
# 問題を回避する。未設定の場合(Windows等、問題が出ていない環境)は従来通り
# ポータブルパッケージ内のdata/vectordb/を使う。
def resolve_vectordb_dir(project_root: Path) -> Path:
    override = os.environ.get("SHIORI_VECTORDB_DIR")
    if override:
        return Path(override)
    return project_root / "data" / "vectordb"
