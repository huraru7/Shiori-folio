"""library/ 配下(profile/garden/portfolioのサブフォルダを含め再帰的)の
Markdownをチャンク分割し、embedding-server経由でベクトル化してChromaDBに
投入する(一度実行するスクリプト)。

実行: .venv\\Scripts\\python.exe ingest.py
"""
from __future__ import annotations

from pathlib import Path

import chromadb
import httpx

from chunking import chunk_markdown
from embedding_client import get_embedding

PROJECT_ROOT = Path(__file__).resolve().parents[2]
# system/library分離により、知識データ(library/)はsystem/(PROJECT_ROOT)の
# 外、その兄弟ディレクトリに置かれている。
KNOWLEDGE_DIR = PROJECT_ROOT.parent / "library"
VECTORDB_DIR = PROJECT_ROOT / "data" / "vectordb"
COLLECTION_NAME = "shiori_knowledge"


def source_category_for(md_path: Path) -> str:
    """library/直下のサブフォルダ名(profile/garden/portfolio等)をカテゴリとする。
    サブフォルダ無しでlibrary/直下に置かれたファイルは"uncategorized"扱い。
    """
    relative = md_path.relative_to(KNOWLEDGE_DIR)
    return relative.parts[0] if len(relative.parts) > 1 else "uncategorized"


def main() -> None:
    client = chromadb.PersistentClient(path=str(VECTORDB_DIR))
    # 再実行しても重複投入しないよう、毎回コレクションを作り直す
    try:
        client.delete_collection(COLLECTION_NAME)
    except Exception:
        pass
    collection = client.create_collection(COLLECTION_NAME)

    md_files = sorted(KNOWLEDGE_DIR.rglob("*.md"))
    if not md_files:
        raise SystemExit(f"Markdownファイルが見つかりません: {KNOWLEDGE_DIR}")

    ids: list[str] = []
    documents: list[str] = []
    embeddings: list[list[float]] = []
    metadatas: list[dict] = []

    with httpx.Client(timeout=30.0) as http_client:
        for md_path in md_files:
            text = md_path.read_text(encoding="utf-8")
            category = source_category_for(md_path)
            chunks = chunk_markdown(text)
            for i, chunk in enumerate(chunks):
                chunk_id = f"{category}-{md_path.stem}-{i}"
                # 見出しには「huraru.comのベースカラー」のように、そのまま質問の
                # 言い回しに近い言葉が入っていることが多い。本文だけを埋め込むと
                # その情報が失われ、特に見出しを細かく割った短いチャンクで検索精度が
                # 落ちるため、見出しを前置きして埋め込む(表示用documentsは本文のまま)。
                embed_text = (
                    f"{chunk.heading}: {chunk.text}"
                    if chunk.heading != "(見出しなし)"
                    else chunk.text
                )
                embedding = get_embedding(embed_text, client=http_client, is_query=False)
                ids.append(chunk_id)
                documents.append(chunk.text)
                embeddings.append(embedding)
                metadatas.append(
                    {
                        "source": md_path.name,
                        "heading": chunk.heading,
                        "source_category": category,
                    }
                )
                print(f"投入: {chunk_id} ({chunk.heading})")

    collection.add(ids=ids, documents=documents, embeddings=embeddings, metadatas=metadatas)
    print(f"完了: {len(ids)}件のチャンクを投入しました")


if __name__ == "__main__":
    main()
