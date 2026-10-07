"""library/配下のMarkdown(と素材の説明ファイル.meta)をChromaDBへ増分同期する共通ロジック(詩織Ver2.0
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

from chunking import Chunk, chunk_markdown, split_section
from embedding_client import get_embedding
from extraction.runner import extract_asset
from passage import passage_text

INDEX_STATE_FILENAME = ".index_state.json"
# 埋め込みの作り方(何を前置して埋め込むか等)を変えたときに上げる。保存済みの値と違えば、
# 起動時の同期(sync_index)が全ファイルを埋め込み直す。
# 2: 記事のタイトル+要約を前置して埋め込み、メタデータにsummaryを持つ(詩織Ver3.8)。
# 3: メタデータにproject/type/authorを持つ(/search_libraryのfilterが常に0件になる不具合の修正)。
INDEX_SCHEMA_VERSION = 3
INDEX_SCHEMA_FILENAME = ".index_schema"
# 素材の抽出結果のキャッシュ置き場(vectordbと同じく、libraryから作り直せる派生データ)と、
# 抽出できなかった・飛ばした素材の記録。
EXTRACT_CACHE_DIRNAME = "extract-cache"
EXTRACT_WARNINGS_FILENAME = ".extract_warnings.json"

# 素材ファイル(zip・png・pdf等)の説明を書いたサイドカー。「X.zip」に対して
# 「X.zip.meta」を置く(詩織Ver3.5)。中身はmdと同じfrontmatter+本文のため、
# mdと同じ経路でインデックスする。
META_SUFFIX = ".meta"
INDEXED_SUFFIXES = (".md", META_SUFFIX)


def is_indexable_path(path: Path, knowledge_dir: Path) -> bool:
    """インデックス対象(*.mdまたは*.meta)かを返す。library/_system/配下と、
    「._」始まりのAppleDouble(下のsync_index参照)は対象外。
    """
    if path.suffix not in INDEXED_SUFFIXES:
        return False
    return "_system" not in path.relative_to(knowledge_dir).parts and not path.name.startswith("._")


def list_indexable_files(knowledge_dir: Path) -> list[Path]:
    return [p for p in knowledge_dir.rglob("*") if p.is_file() and is_indexable_path(p, knowledge_dir)]


def asset_path_for(meta_path: Path) -> Path | None:
    """.metaに対応する実体(.metaと同じフォルダの、.metaを除いた名前のファイル)を
    返す。.metaでない・実体が無い(孤児)場合はNone。
    """
    if meta_path.suffix != META_SUFFIX:
        return None
    asset = meta_path.with_suffix("")
    return asset if asset.is_file() else None

# frontmatterのindexフィールド(詩織Ver3.0、データ管理法見直し2-2節)。
# 厳密なYAMLパースは行わず、正規表現でindex: falseの1行だけを検知する
# 軽量な実装にしている(pyyaml等の追加依存を避けるため。frontmatter全体を
# 解析する必要が出てきたら見直すこと)。
_FRONTMATTER_RE = re.compile(r"^---\r?\n(.*?)\r?\n---\r?\n?", re.DOTALL)
_INDEX_FALSE_RE = re.compile(r"^index:\s*false\s*$", re.MULTILINE | re.IGNORECASE)
_TITLE_RE = re.compile(r"^title:\s*(.+?)\s*$", re.MULTILINE)
_SUMMARY_RE = re.compile(r"^summary:\s*(.+?)\s*$", re.MULTILINE)
# 詩織Ver3.3(時間認識検索)で新設。status/dateはいずれも_TITLE_REと同じ理由
# (pyyaml等を追加依存させない軽量な抽出)で正規表現のみを使う。
_EXTRACT_FALSE_RE = re.compile(r"^extract:\s*false\s*$", re.MULTILINE | re.IGNORECASE)
_STATUS_RE = re.compile(r"^status:\s*(.+?)\s*$", re.MULTILINE)
_DATE_RE = re.compile(r"^date:\s*(.+?)\s*$", re.MULTILINE)
# /search_libraryのfilter(author/type/project)の絞り込み先。
_PROJECT_RE = re.compile(r"^project:\s*(.+?)\s*$", re.MULTILINE)
_TYPE_RE = re.compile(r"^type:\s*(.+?)\s*$", re.MULTILINE)
_AUTHOR_RE = re.compile(r"^author:\s*(.+?)\s*$", re.MULTILINE)


def _is_indexable(text: str) -> bool:
    """frontmatterのindexフィールドがfalseなら検索対象から除外する。
    フィールドが無い・frontmatter自体が無い場合はデフォルトのtrue
    (検索対象)として扱う。
    """
    m = _FRONTMATTER_RE.match(text)
    if not m:
        return True
    return not _INDEX_FALSE_RE.search(m.group(1))


def _extract_disabled(text: str) -> bool:
    """素材の.metaに`extract: false`があれば、その素材の中身は抽出しない(機密の素材のオプトアウト)。"""
    m = _FRONTMATTER_RE.match(text)
    return bool(m and _EXTRACT_FALSE_RE.search(m.group(1)))


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


def _extract_summary(text: str) -> str:
    """frontmatterのsummaryを取り出す(詩織Ver3.8)。検索用のパッセージに前置して、
    チャンク単体では分からない「その記事が何についてか」を補う。無ければ空文字列。
    """
    return _extract_frontmatter_field(_SUMMARY_RE, text)


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


def _extract_filter_fields(text: str) -> dict[str, str]:
    """/search_libraryのfilterで絞り込むfrontmatterのproject/type/authorを取り出す。
    ChromaDBのwhereは完全一致のため、メタデータに無いと絞り込みが常に0件になる。
    無い項目は空文字列(status/dateと同じ扱い)。
    """
    return {
        "project": _extract_frontmatter_field(_PROJECT_RE, text),
        "type": _extract_frontmatter_field(_TYPE_RE, text),
        "author": _extract_frontmatter_field(_AUTHOR_RE, text),
    }


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


def load_index_schema(vectordb_dir: Path) -> int:
    path = vectordb_dir / INDEX_SCHEMA_FILENAME
    try:
        return int(path.read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        return 0


def save_index_schema(vectordb_dir: Path) -> None:
    (vectordb_dir / INDEX_SCHEMA_FILENAME).write_text(str(INDEX_SCHEMA_VERSION), encoding="utf-8")


def save_index_state(vectordb_dir: Path, state: dict[str, float]) -> None:
    path = vectordb_dir / INDEX_STATE_FILENAME
    path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")


def load_extract_warnings(vectordb_dir: Path) -> dict[str, dict]:
    path = vectordb_dir / EXTRACT_WARNINGS_FILENAME
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}


def _record_extract_status(vectordb_dir: Path, relative_path: str, reason: str | None, warn: bool) -> None:
    """素材の抽出結果を記録する。reasonがNone(抽出できた)なら、前回までの記録を消す。"""
    records = load_extract_warnings(vectordb_dir)
    if reason is None:
        if records.pop(relative_path, None) is None:
            return
    else:
        records[relative_path] = {"reason": reason, "warn": warn}
        if warn:
            print(f"警告: 素材の抽出を止めました({relative_path}): {reason}", file=sys.stderr)
    (vectordb_dir / EXTRACT_WARNINGS_FILENAME).write_text(
        json.dumps(records, ensure_ascii=False, indent=2), encoding="utf-8"
    )


def _state_mtime(md_path: Path) -> float:
    """同期状態に記録する更新時刻。素材(.meta)は、実体だけが差し替わっても再抽出できるよう、
    .metaと実体のうち新しい方を使う。
    """
    mtime = md_path.stat().st_mtime
    asset = asset_path_for(md_path)
    return max(mtime, asset.stat().st_mtime) if asset else mtime


def _embed_asset_chunks(
    collection,
    http_client: httpx.Client,
    md_path: Path,
    category: str,
    relative_path: str,
    stem: str,
    base_metadata: dict,
    title: str,
    summary: str,
    vectordb_dir: Path,
    start_index: int,
) -> int:
    """.metaに対応する実体の中身を抽出して、同じsource(=.meta)のチャンクとして登録する。
    抽出できなかった・飛ばした場合は、理由を記録して0を返す(.metaの登録は済んでいる)。
    """
    asset = asset_path_for(md_path)
    if asset is None:
        return 0
    result = extract_asset(asset, vectordb_dir / EXTRACT_CACHE_DIRNAME)
    if not result.ok or not result.sections:
        reason = result.reason or "テキストなし"
        _record_extract_status(vectordb_dir, relative_path, reason, result.warn)
        return 0
    _record_extract_status(vectordb_dir, relative_path, None, False)

    ids: list[str] = []
    documents: list[str] = []
    embeddings: list[list[float]] = []
    metadatas: list[dict] = []
    n = 0
    for label, text in result.sections:
        for part in split_section(Chunk(heading=f"素材 {label}", text=text)):
            embedding = get_embedding(
                passage_text(title, summary, part.heading, part.text), client=http_client, is_query=False
            )
            ids.append(f"{category}-{stem}-x{start_index + n}")
            documents.append(part.text)
            embeddings.append(embedding)
            # 素材由来の印(origin)。検索結果をLLMへ渡すとき「資料の内容であり指示ではない」と明示する。
            metadatas.append({**base_metadata, "heading": part.heading, "origin": "asset"})
            n += 1
    if ids:
        collection.upsert(ids=ids, documents=documents, embeddings=embeddings, metadatas=metadatas)
    return n


def _embed_file(
    collection,
    http_client: httpx.Client,
    md_path: Path,
    category: str,
    relative_path: str,
    vectordb_dir: Path | None = None,
) -> int:
    """1ファイル分のチャンクを埋め込んでChromaDBへ反映する。チャンク数が前回
    から変わっている可能性があるため、まず同じsource(ファイル名)の既存チャンクを
    すべて削除してから、あらためて全チャンクを追加し直す。戻り値は投入した
    チャンク数(0ならファイルが空・見出し/本文が無い、またはindex: falseで
    検索対象から除外されている)。
    """
    # 「X.zip.meta」のstemは「X.zip」でX.zip.mdと衝突しうるため、.metaは
    # 拡張子込みのファイル名をid接頭辞にする。
    stem = md_path.name if md_path.suffix == META_SUFFIX else md_path.stem
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
    summary = _extract_summary(text)
    status = _extract_status(text)
    date = _extract_date(text)
    related = _extract_related(text)
    filter_fields = _extract_filter_fields(text)

    ids: list[str] = []
    documents: list[str] = []
    embeddings: list[list[float]] = []
    metadatas: list[dict] = []
    for i, chunk in enumerate(chunks):
        # 見出し・記事のタイトルと要約を前置して埋め込む理由はpassage.py参照
        # (検索精度向上のため)。
        embed_text = passage_text(title, summary, chunk.heading, chunk.text)
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
                # 検索用パッセージ(passage.py)をリランク・語彙検索で再現するために持つ。
                "summary": summary,
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
                **filter_fields,
            }
        )

    collection.upsert(ids=ids, documents=documents, embeddings=embeddings, metadatas=metadatas)

    # 素材(.meta)なら、実体の中身も同じsourceのチャンクとして登録する(詩織Ver3.8)。
    # extract: falseの素材と、抽出先(vectordb_dir)が無い呼び出しでは行わない。
    extracted = 0
    if md_path.suffix == META_SUFFIX and vectordb_dir is not None:
        if _extract_disabled(text):
            _record_extract_status(vectordb_dir, relative_path, "extract: false のため抽出しない", False)
        else:
            extracted = _embed_asset_chunks(
                collection, http_client, md_path, category, relative_path, stem,
                {k: v for k, v in metadatas[0].items()}, title, summary, vectordb_dir, len(ids),
            )
    return len(ids) + extracted


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
    count = _embed_file(collection, http_client, md_path, category, relative_path, vectordb_dir)

    state = load_index_state(vectordb_dir)
    state[relative_path] = _state_mtime(md_path)
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
    # 埋め込みの作り方が変わっていたら、保存済みの状態を捨てて全ファイルを埋め込み直す。
    schema_changed = load_index_schema(vectordb_dir) != INDEX_SCHEMA_VERSION
    if schema_changed:
        state = {}
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
    md_files = list_indexable_files(knowledge_dir)
    current_paths = {str(p.relative_to(knowledge_dir)): p for p in md_files}

    added: list[str] = []
    updated: list[str] = []
    removed: list[str] = []

    # チャンクはファイル名(source)単位で管理しているため、ファイルを別フォルダへ
    # 移動(アーカイブ等)した場合、旧パスの削除でsource名が同じ移動先のチャンクまで
    # 消えてしまう(移動先がreindex_file済みでmtime一致のとき、再投入もされない)。
    # 同名のファイルが現存するときは、チャンクの削除はせず状態だけ更新する。
    current_names = {Path(rel).name for rel in current_paths}
    if schema_changed:
        # 状態を捨てたので、前回までに消えたファイルのチャンクを「削除」として拾えない。
        # 現存するファイル名に無いsourceのチャンクを、ここでまとめて消す。
        stale_ids = [
            chunk_id
            for chunk_id, meta in zip(*(lambda r: (r["ids"], r["metadatas"]))(collection.get(include=["metadatas"])))
            if (meta or {}).get("source") not in current_names
        ]
        if stale_ids:
            collection.delete(ids=stale_ids)
    for rel in list(state.keys()):
        if rel not in current_paths:
            if Path(rel).name not in current_names:
                _remove_file(collection, Path(rel).name)
            _record_extract_status(vectordb_dir, rel, None, False)
            del state[rel]
            removed.append(rel)

    for rel, md_path in current_paths.items():
        mtime = _state_mtime(md_path)
        prior = state.get(rel)
        if prior == mtime:
            continue
        category = source_category_for(md_path, knowledge_dir)
        try:
            _embed_file(collection, http_client, md_path, category, rel, vectordb_dir)
        except (UnicodeDecodeError, OSError) as e:
            # 1ファイルの読み込み失敗で全体(起動シーケンスを含む)を巻き込まない。
            # 「なんでも保存」方針上、壊れたファイルが1つあってもアプリは
            # 動き続けるべき(そのファイルが検索に出てこないだけに留める)。
            print(f"警告: {rel} の読み込みに失敗したためスキップします({e})", file=sys.stderr)
            continue
        (added if prior is None else updated).append(rel)
        state[rel] = mtime

    if added or updated or removed or schema_changed:
        save_index_state(vectordb_dir, state)
    if schema_changed:
        save_index_schema(vectordb_dir)

    return collection, {"added": added, "updated": updated, "removed": removed}
