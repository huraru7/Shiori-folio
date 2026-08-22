"""library/配下のMarkdownをChromaDBへ増分同期する共通ロジック(詩織Ver2.0
設計指示書v3、4章「埋め込みデーモンの起動ロジック+検索時の遅延再インデックス」)。

ingest.py(手動でのコレクション全体再構築)とapp.py(検索リクエストのたびに
呼ぶ遅延再インデックス、Phase 6で追加)の両方から使う共通処理をここに集約する。
"""
from __future__ import annotations

import json
from pathlib import Path

import chromadb
import httpx

from chunking import chunk_markdown
from embedding_client import get_embedding

INDEX_STATE_FILENAME = ".index_state.json"


def source_category_for(md_path: Path, knowledge_dir: Path) -> str:
    """library/直下のサブフォルダ名(20-projects等)をカテゴリとする。
    サブフォルダ無しでlibrary/直下に置かれたファイルは"uncategorized"扱い。
    """
    relative = md_path.relative_to(knowledge_dir)
    return relative.parts[0] if len(relative.parts) > 1 else "uncategorized"


def load_index_state(vectordb_dir: Path) -> dict[str, float]:
    """前回同期時点での{library/からの相対パス: mtime}を読み込む。
    状態ファイルが無い・壊れている場合は空辞書を返す(次回の同期で全件を
    新規扱いとして再構築されるだけなので、壊れていても安全側に倒れる)。
    """
    path = vectordb_dir / INDEX_STATE_FILENAME
    if not path.is_file():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return {}


def save_index_state(vectordb_dir: Path, state: dict[str, float]) -> None:
    path = vectordb_dir / INDEX_STATE_FILENAME
    path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")


def _embed_file(
    collection, http_client: httpx.Client, md_path: Path, category: str
) -> int:
    """1ファイル分のチャンクを埋め込んでChromaDBへ反映する。チャンク数が前回
    から変わっている可能性があるため、まず同じsource(ファイル名)の既存チャンクを
    すべて削除してから、あらためて全チャンクを追加し直す。戻り値は投入した
    チャンク数(0ならファイルが空、または見出し・本文が無い)。
    """
    stem = md_path.stem
    existing = collection.get(where={"source": md_path.name})
    if existing["ids"]:
        collection.delete(ids=existing["ids"])

    text = md_path.read_text(encoding="utf-8")
    chunks = chunk_markdown(text)
    if not chunks:
        return 0

    ids: list[str] = []
    documents: list[str] = []
    embeddings: list[list[float]] = []
    metadatas: list[dict] = []
    for i, chunk in enumerate(chunks):
        # 見出しを前置きして埋め込む理由はingest.py参照(検索精度向上のため)。
        embed_text = (
            f"{chunk.heading}: {chunk.text}" if chunk.heading != "(見出しなし)" else chunk.text
        )
        embedding = get_embedding(embed_text, client=http_client, is_query=False)
        ids.append(f"{category}-{stem}-{i}")
        documents.append(chunk.text)
        embeddings.append(embedding)
        metadatas.append(
            {"source": md_path.name, "heading": chunk.heading, "source_category": category}
        )

    collection.upsert(ids=ids, documents=documents, embeddings=embeddings, metadatas=metadatas)
    return len(ids)


def _remove_file(collection, source_name: str) -> None:
    existing = collection.get(where={"source": source_name})
    if existing["ids"]:
        collection.delete(ids=existing["ids"])


def reindex_single_file(
    chroma_client: chromadb.ClientAPI,
    http_client: httpx.Client,
    knowledge_dir: Path,
    vectordb_dir: Path,
    collection_name: str,
    relative_path: str,
) -> int:
    """書き込み時フック(shiori-save CLI等)向け。検索の都度の全件mtimeスキャンを
    待たず、保存直後のファイル1件だけを即座に埋め込み直す(詩織Ver3.0、
    データ管理法見直し2-5節)。.index_state.jsonのmtimeもここで更新しておく
    ことで、次回起動時の全件スキャン(sync_index)がこのファイルを「変更あり」
    と誤検知して二重に埋め込み直すのを防ぐ。戻り値は投入したチャンク数。
    """
    collection = chroma_client.get_or_create_collection(collection_name)
    md_path = knowledge_dir / relative_path
    if not md_path.is_file():
        raise FileNotFoundError(relative_path)

    category = source_category_for(md_path, knowledge_dir)
    count = _embed_file(collection, http_client, md_path, category)

    state = load_index_state(vectordb_dir)
    state[relative_path] = md_path.stat().st_mtime
    save_index_state(vectordb_dir, state)
    return count


def sync_index(
    chroma_client: chromadb.ClientAPI,
    http_client: httpx.Client,
    knowledge_dir: Path,
    vectordb_dir: Path,
    collection_name: str,
):
    """knowledge_dir配下のファイルmtimeを前回の同期状態と比較し、新規・変更・
    削除されたファイルだけを増分でChromaDBに反映する(全件embedding計算の
    やり直しはしない)。呼び出しコストはファイルの列挙とstat()が支配的で、
    変更が無ければembeddingサーバーへは一切問い合わせない(検索リクエストの
    たびに呼んでも実用上のオーバーヘッドは小さい想定)。

    戻り値は(collection, {"added": [...], "updated": [...], "removed": [...]})
    (いずれも library/ からの相対パスのリスト)。
    """
    collection = chroma_client.get_or_create_collection(collection_name)

    state = load_index_state(vectordb_dir)
    # library/_system/配下はConversationの保存ルール(CLAUDE.md)・タグ/プロジェクト
    # 台帳(tags.yaml/projects.yaml)等の設定ファイル置き場であり、蔵書ではない。
    # 除外せずrglobすると_system/CLAUDE.mdまで検索結果・図書館UIに紛れ込み、
    # かつsource_category="_system"は図書館UI側の分類マッピングに存在しないため
    # 「未分類」として表示されてしまう(2026-08-17、Windows実機のVer2.0移行後
    # 確認で発覚)。
    md_files = [
        p for p in knowledge_dir.rglob("*.md") if "_system" not in p.relative_to(knowledge_dir).parts
    ]
    current_paths = {str(p.relative_to(knowledge_dir)): p for p in md_files}

    added: list[str] = []
    updated: list[str] = []
    removed: list[str] = []

    for rel in list(state.keys()):
        if rel not in current_paths:
            _remove_file(collection, Path(rel).name)
            del state[rel]
            removed.append(rel)

    for rel, md_path in current_paths.items():
        mtime = md_path.stat().st_mtime
        prior = state.get(rel)
        if prior == mtime:
            continue
        category = source_category_for(md_path, knowledge_dir)
        _embed_file(collection, http_client, md_path, category)
        (added if prior is None else updated).append(rel)
        state[rel] = mtime

    if added or updated or removed:
        save_index_state(vectordb_dir, state)

    return collection, {"added": added, "updated": updated, "removed": removed}
