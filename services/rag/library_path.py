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

from pathlib import Path


def resolve_knowledge_dir(project_root: Path) -> Path:
    portable_library = project_root / "library"
    if portable_library.is_dir():
        return portable_library
    return project_root.parent / "library"
