"""library/配下のMarkdownをChromaDBへ増分同期する共通ロジック(詩織Ver2.0
設計指示書v3、4章「埋め込みデーモンの起動ロジック+検索時の遅延再インデックス」)。

ingest.py(手動でのコレクション全体再構築)とapp.py(検索リクエストのたびに
呼ぶ遅延再インデックス、Phase 6で追加)の両方から使う共通処理をここに集約する。
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

import chromadb
import httpx

from chunking import chunk_markdown
from embedding_client import get_embedding

INDEX_STATE_FILENAME = ".index_state.json"

# frontmatterのindexフィールド(詩織Ver3.0、データ管理法見直し2-2節)。
# 厳密なYAMLパースは行わず、正規表現でindex: falseの1行だけを検知する
# 軽量な実装にしている(pyyaml等の追加依存を避けるため。frontmatter全体を
# 解析する必要が出てきたら見直すこと)。
_FRONTMATTER_RE = re.compile(r"^---\r?\n(.*?)\r?\n---\r?\n?", re.DOTALL)
_INDEX_FALSE_RE = re.compile(r"^index:\s*false\s*$", re.MULTILINE | re.IGNORECASE)
_TITLE_RE = re.compile(r"^title:\s*(.+?)\s*$", re.MULTILINE)
# 詩織Ver3.3(時間認識検索)で新設。status/dateはいずれも_TITLE_REと同じ理由
# (pyyaml等を追加依存させない軽量な抽出)で正規表現のみを使う。
_STATUS_RE = re.compile(r"^status:\s*(.+?)\s*$", re.MULTILINE)
_DATE_RE = re.compile(r"^date:\s*(.+?)\s*$", re.MULTILINE)


def _is_indexable(text: str) -> bool:
    """frontmatterのindexフィールドがfalseなら検索対象から除外する。
    フィールドが無い・frontmatter自体が無い場合はデフォルトのtrue
    (検索対象)として扱う。
    """
    m = _FRONTMATTER_RE.match(text)
    if not m:
        return True
    return not _INDEX_FALSE_RE.search(m.group(1))


def _strip_quotes(raw: str) -> str:
    """値がダブル/シングルクォートで囲まれている場合(コロンを含む値は
    クォート必須)に剥がす。frontmatterの各フィールド抽出で共通して使う。
    """
    if len(raw) >= 2 and raw[0] == raw[-1] and raw[0] in "\"'":
        return raw[1:-1]
    return raw


def _extract_frontmatter_field(pattern: re.Pattern[str], text: str) -> str:
    m = _FRONTMATTER_RE.match(text)
    if not m:
        return ""
    field_m = pattern.search(m.group(1))
    if not field_m:
        return ""
    return _strip_quotes(field_m.group(1))


def _extract_title(text: str) -> str:
    """frontmatterのtitleを取り出す(GUIの一覧表示用、2026-09-16追加)。
    _is_indexableと同じ理由でpyyaml等は使わず正規表現で軽量に済ませる。
    見つからなければ空文字列(呼び出し側でファイル名にフォールバックする)。
    """
    return _extract_frontmatter_field(_TITLE_RE, text)


def _extract_status(text: str) -> str:
    """frontmatterのstatusを取り出す(詩織Ver3.3、時間認識検索で新設)。
    見つからなければ空文字列(shiori_save.rsのデフォルト値"new"を前提とせず、
    呼び出し側で明示的に扱う)。
    """
    return _extract_frontmatter_field(_STATUS_RE, text)


def _extract_date(text: str) -> str:
    """frontmatterのdate(ISO8601)を取り出す(詩織Ver3.3、時間認識検索で新設)。
    見つからなければ空文字列(date必須化前の記事が万一残っていた場合の
    フォールバックで、呼び出し側は空文字列を「不明」として扱う)。
    """
    return _extract_frontmatter_field(_DATE_RE, text)


def _extract_related(text: str) -> list[str]:
    """frontmatterのrelated(文字列配列)を取り出す(詩織Ver3.3、時間認識検索
    で新設。新旧記事の手がかりとして経緯モードの検索結果に含める)。
    shiori_save.rs(serde_yaml)が出力する2形式(ブロックスタイルの`- item`列と、
    空配列の`[]`)に対応する。他フィールドと同じくpyyaml等は使わない軽量な
    行ベースの抽出に留める。
    """
    m = _FRONTMATTER_RE.match(text)
    if not m:
        return []
    lines = m.group(1).splitlines()
    for i, line in enumerate(lines):
        stripped = line.strip()
        if not stripped.startswith("related:"):
            continue
        inline = stripped[len("related:") :].strip()
        if inline.startswith("["):
            inner = inline.strip("[]").strip()
            return [_strip_quotes(item.strip()) for item in inner.split(",") if item.strip()]
        items = []
        for next_line in lines[i + 1 :]:
            next_stripped = next_line.strip()
            if next_stripped.startswith("- "):
                items.append(_strip_quotes(next_stripped[2:].strip()))
            elif next_stripped == "":
                continue
            else:
                break
        return items
    return []


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
    collection, http_client: httpx.Client, md_path: Path, category: str, relative_path: str
) -> int:
    """1ファイル分のチャンクを埋め込んでChromaDBへ反映する。チャンク数が前回
    から変わっている可能性があるため、まず同じsource(ファイル名)の既存チャンクを
    すべて削除してから、あらためて全チャンクを追加し直す。戻り値は投入した
    チャンク数(0ならファイルが空・見出し/本文が無い、またはindex: falseで
    検索対象から除外されている)。
    """
    stem = md_path.stem
    existing = collection.get(where={"source": md_path.name})
    if existing["ids"]:
        collection.delete(ids=existing["ids"])

    text = md_path.read_text(encoding="utf-8")
    if not _is_indexable(text):
        return 0
    chunks = chunk_markdown(text)
    if not chunks:
        return 0

    title = _extract_title(text)
    status = _extract_status(text)
    date = _extract_date(text)
    related = _extract_related(text)

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
            {
                "source": md_path.name,
                "heading": chunk.heading,
                "source_category": category,
                "title": title,
                # library_rootからの相対パス(/区切りに統一、2026-09-16追加)。
                # GUIのエクスプローラー風ツリー表示が、絶対パス文字列の解析
                # という脆い方法に頼らずフォルダ階層を安全に構築するために使う。
                "relative_path": relative_path.replace("\\", "/"),
                # ファイルの更新日時(Unixタイムスタンプ、2026-09-16追加)。
                # エクスプローラー風UIの「更新日時」列に使う。
                "mtime": md_path.stat().st_mtime,
                # frontmatterのstatus/date(詩織Ver3.3、時間認識検索で新設)。
                # 現在モード/経緯モードのスコアリング・ソートに使う
                # (app.py参照)。
                "status": status,
                "date": date,
                # ChromaDBのmetadataはスカラー値のみ受け付けるため、リストは
                # "|"区切りの1文字列に畳んで保存する(app.py側でsplitして復元)。
                "related": "|".join(related),
            }
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
    count = _embed_file(collection, http_client, md_path, category, relative_path)

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
    # 「._」始まりはmacOS/exFAT環境でFinder等が自動生成するAppleDoubleの
    # リソースフォーク・サイドカーファイル(バイナリ、UTF-8ではない)。除外
    # しないとread_text(utf-8)がUnicodeDecodeErrorで落ち、起動時の全件
    # スキャン(このsync_index)がアプリ起動そのものを道連れにしてしまう
    # (2026-08-25、実機でRAGサーバーが起動直後にクラッシュする不具合として発覚)。
    md_files = [
        p
        for p in knowledge_dir.rglob("*.md")
        if "_system" not in p.relative_to(knowledge_dir).parts and not p.name.startswith("._")
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
        try:
            _embed_file(collection, http_client, md_path, category, rel)
        except (UnicodeDecodeError, OSError) as e:
            # 1ファイルの読み込み失敗で全体(起動シーケンスを含む)を巻き込まない。
            # 「なんでも保存」方針上、壊れたファイルが1つあってもアプリは
            # 動き続けるべき(そのファイルが検索に出てこないだけに留める)。
            print(f"警告: {rel} の読み込みに失敗したためスキップします({e})", file=sys.stderr)
            continue
        (added if prior is None else updated).append(rel)
        state[rel] = mtime

    if added or updated or removed:
        save_index_state(vectordb_dir, state)

    return collection, {"added": added, "updated": updated, "removed": removed}
