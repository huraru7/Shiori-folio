"""library/ 配下(profile/garden/portfolioのサブフォルダを含め再帰的)の
Markdownをチャンク分割し、embedding-server経由でベクトル化してChromaDBに
投入する(コレクションを作り直す、手動実行用のフル再構築スクリプト)。

通常の運用では検索リクエストのたびにapp.py側で増分の遅延再インデックス
(indexing.py、Phase 6)が走るため、このスクリプトを都度実行する必要は無い。
チャンク分割ロジックの変更等でコレクション全体を作り直したい場合にのみ使う。

実行: .venv\\Scripts\\python.exe ingest.py
"""
from __future__ import annotations

from pathlib import Path

import chromadb
import httpx

from indexing import save_index_state, sync_index

PROJECT_ROOT = Path(__file__).resolve().parents[2]
# system/library分離により、知識データ(library/)はsystem/(PROJECT_ROOT)の
# 外、その兄弟ディレクトリに置かれている。
KNOWLEDGE_DIR = PROJECT_ROOT.parent / "library"
VECTORDB_DIR = PROJECT_ROOT / "data" / "vectordb"
COLLECTION_NAME = "shiori_knowledge"


def main() -> None:
    client = chromadb.PersistentClient(path=str(VECTORDB_DIR))
    # 再実行しても重複投入しないよう、毎回コレクションを作り直す。
    try:
        client.delete_collection(COLLECTION_NAME)
    except Exception:
        pass

    if not any(KNOWLEDGE_DIR.rglob("*.md")):
        raise SystemExit(f"Markdownファイルが見つかりません: {KNOWLEDGE_DIR}")

    # 同期状態を空にリセットしてから増分同期を呼ぶことで、実質的に全件投入になる
    # (sync_index自体は増分専用だが、コレクションを空にした直後に呼べば
    # 「前回の状態が空」=「全ファイルが新規」として扱われる)。
    save_index_state(VECTORDB_DIR, {})

    with httpx.Client(timeout=30.0) as http_client:
        _collection, changes = sync_index(
            client, http_client, KNOWLEDGE_DIR, VECTORDB_DIR, COLLECTION_NAME
        )

    for rel in changes["added"]:
        print(f"投入: {rel}")
    print(f"完了: {len(changes['added'])}件のファイルを投入しました")


if __name__ == "__main__":
    main()
