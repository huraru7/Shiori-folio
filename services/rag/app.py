"""RAG検索サーバー。/search でクエリを受け取り、embedding-server経由で
ベクトル化してChromaDBのshiori_knowledgeコレクションをtop_k件検索する。

起動: Windowsは.venv\\Scripts\\uvicorn.exe、Mac/Linuxは.venv/bin/uvicorn を使い
app:app --port 8083 --host 127.0.0.1
"""
from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path
from typing import Literal

import chromadb
import httpx
from fastapi import FastAPI
from pydantic import BaseModel

from embedding_client import get_embedding
from indexing import reindex_single_file, sync_index
from library_path import resolve_knowledge_dir
from reranker import rerank, warmup as warmup_reranker

PROJECT_ROOT = Path(__file__).resolve().parents[2]
# 開発ツリー/ポータブル版どちらでも正しいlibrary/を指すよう、
# library_path.resolve_knowledge_dir()に判定を委譲する(詳細はそちら参照)。
KNOWLEDGE_DIR = resolve_knowledge_dir(PROJECT_ROOT)
VECTORDB_DIR = PROJECT_ROOT / "data" / "vectordb"
COLLECTION_NAME = "shiori_knowledge"

app = FastAPI()
_chroma_client = chromadb.PersistentClient(path=str(VECTORDB_DIR))
_http_client = httpx.Client(timeout=30.0)


# リランカー(CrossEncoder)は初回呼び出し時に遅延ロードされる設計だが、それだと
# ユーザーの初回検索が約6秒ブロックされる(2026-08-14、外付けSSD運用時の調査で
# 発覚)。uvicornはlifespan startupイベントが完了するまでリクエストを受け付けない
# ため、ここでロードしておくことで/healthが返る時点(=Tauri起動画面の待機処理が
# 見ているタイミング)には既にロード済みの状態にできる。
@app.on_event("startup")
def _warmup_reranker() -> None:
    warmup_reranker()


# 検索の都度の全件mtimeスキャン(旧・遅延再インデックス方式)を廃止し、書き込み
# 時フック(/reindex_file、shiori-save CLI等が保存直後に呼ぶ)を主軸にする
# (詩織Ver3.0、データ管理法見直し2-5節)。起動時にはここで1回だけ全件スキャンを
# 行い、フックの取りこぼしやポータブルSSDを別マシンで直接編集したケースを
# 補完する安全網とする。
@app.on_event("startup")
def _initial_index_sync() -> None:
    sync_index(_chroma_client, _http_client, KNOWLEDGE_DIR, VECTORDB_DIR, COLLECTION_NAME)


class SearchRequest(BaseModel):
    query: str
    top_k: int = 5
    # 詩織Ver3.3(時間認識検索)で新設。"current"(デフォルト)/"history"は
    # _retrieve_and_rerank参照。
    mode: Literal["current", "history"] = "current"


class SearchResultItem(BaseModel):
    # ChromaDBのchunk ID(ingest.pyの`{category}-{stem}-{i}`形式)。RAGAS評価で
    # 「どのチャンクが検索されたか」を機械的に判定するために追加(2026-08-07)。
    id: str
    text: str
    source: str
    heading: str
    distance: float
    # 日本語Cross-Encoderによるリランキングスコア(2026-08-07追加)。
    # embedding距離だけでは、この規模のデータで正解チャンクと無関係な
    # チャンクの距離が逆転することがあると実測で分かったため導入した
    # (docs/voice-consistency-policy.md 4a節参照)。0〜1に近い値で、
    # 高いほど関連度が高い。
    rerank_score: float
    # library/直下のサブフォルダ名(profile/garden/portfolio等)。
    # 将来の「参照資料の表示モーダル」等での出どころ別絞り込み用。
    source_category: str = "uncategorized"


# ベクトル検索で広めに候補を取ってからリランクする候補プールの下限。
# 呼び出し側のtop_kがこれより小さい場合でも、この件数までは候補を見る。
#
# 当初10件で試したところ、「詩織の音声認識は何を使ってるの？」等、正解
# チャンクがembedding距離では13〜17位・「huraru.comの参照サイトは？」では
# 12位相当まで下がるケースがあり、候補プールに入らずリランカーが正解を
# 見る機会すら無い(リランクは候補プール内の順位しか変えられない)ことが
# 実測で判明したため30に広げた。その後、メモ機能(2026-08-08追加)の実機
# 確認で、「さっきメモした〜って何だっけ」のような言い回しだと、保存した
# メモが30位以内にすら入らない(=リランカーが候補を見る機会が無い)ケースが
# 見つかった。この規模のデータでは埋め込みの第一段階だけで足切りすること
# 自体が不安定さの温床になっている。
#
# そこで、固定値ではなくChromaDBの実際の総件数を上限として使うことにした。
# 「全件を候補にしてリランカーに判断させる」方式で、現状のデータ規模
# (40件程度)なら全件リランクでも実測150ms程度に収まる。ただし件数が
# 大きく増えるとこの前提が崩れる(全件リランクが遅くなる)ため、安全弁として
# RERANK_CANDIDATE_POOL_MAXで上限をかけている。**この上限に達するほど
# データが増えた場合は、全件リランク方式自体を見直すこと**(ハイブリッド
# 検索の導入等、フェーズ2以降で検討する)。
RERANK_CANDIDATE_POOL_MAX = 200

# リランカーのスコアがこれ未満のチャンクは「無関係」として除外する。
# 実測(2026-08-07)では、実際に問われている内容に答えているチャンクは
# だいたい0.9以上、無関係なチャンクは0.05未満、「該当情報が存在しない」
# ケースでの最高スコアは0.02程度だった。この間の0.3を閾値とすることで、
# 明確に関連するチャンクは通しつつ、無関係なチャンクは弾けるようにしている。
RERANK_SCORE_THRESHOLD = 0.3

# 詩織Ver3.3(時間認識検索)。現在モードでの時間減衰の強さ。
# 直近30日は減衰なし(新着記事を不利にしない)、30〜365日は1.0→0.8へ
# 線形に減衰、365日を超えたら0.8で下げ止める(「古いから無価値」には
# しない。あくまでrerank_scoreへの軽い補正に留める)。
_DECAY_GRACE_DAYS = 30
_DECAY_FULL_DAYS = 365
_DECAY_FLOOR = 0.8


def _parse_date(date_str: str) -> datetime | None:
    """frontmatterのdate(ISO8601想定)をdatetimeへ変換する。空文字列・
    パース失敗時はNone(呼び出し側で「日付不明」として扱う)。date必須化
    (詩織Ver3.3)前に保存された記事が万一残っていた場合の安全弁でもある。
    """
    if not date_str:
        return None
    try:
        dt = datetime.fromisoformat(date_str)
    except ValueError:
        return None
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt


def _time_decay_factor(date_str: str) -> float:
    """現在モードでrerank_scoreに掛け合わせる時間減衰係数。日付不明な
    記事は不利にしないよう減衰させない(1.0のまま)。
    """
    dt = _parse_date(date_str)
    if dt is None:
        return 1.0
    days = (datetime.now(timezone.utc) - dt).total_seconds() / 86400
    if days <= _DECAY_GRACE_DAYS:
        return 1.0
    if days >= _DECAY_FULL_DAYS:
        return _DECAY_FLOOR
    progress = (days - _DECAY_GRACE_DAYS) / (_DECAY_FULL_DAYS - _DECAY_GRACE_DAYS)
    return 1.0 - progress * (1.0 - _DECAY_FLOOR)


def _retrieve_and_rerank(
    query: str,
    where: dict | None = None,
    apply_threshold: bool = True,
    mode: Literal["current", "history"] = "current",
) -> list[tuple[str, str, dict, float, float]]:
    """クエリに対する候補チャンクを取得しリランクする。/search・/search_libraryの
    共通処理。返り値はrerank_score降順(chunk_id, document, metadata, distance,
    rerank_score)のリスト。件数の絞り込み(何件返すか)は呼び出し元が行う
    (/searchはチャンク数、/search_libraryはファイル数で数え方が異なるため、
    ここでは絞り込まない)。

    whereはChromaDBのメタデータフィルタ(collection.query()にそのまま渡す)。
    author/type/project等、Ver2.0の保存ルーティング(shiori-save CLI)が
    書き込むフィールドで絞り込む場合に使う。Noneなら絞り込みなし。

    apply_thresholdはRERANK_SCORE_THRESHOLDによる足切りを行うかどうか。
    【Ver3.0で変更】MCP検索(/search_library)は最終消費者がAI(Claude Code)で
    あり、上位から順に読み進めて必要な情報が見つかるまで掘り進められるため、
    「精緻な順位付け」より「関連しうる情報を漏らさず返す(recall重視)」を
    優先する方針に転換した(データ管理法見直し3-1節)。そのため/search_library
    はFalseを渡して閾値を無効化する。/search(会話UI向け、本文をそのまま
    文脈に注入する用途)は無関係な候補が混入する副作用が大きいため、
    従来通りTrue(閾値あり)のままにする。

    modeは詩織Ver3.3(時間認識検索)で新設。"current"(デフォルト)は
    status: deprecatedの記事を除外し、rerank_scoreに時間減衰(新しいほど
    有利)を掛ける。「今どうなっているか」を答える通常の検索向け。
    "history"はdeprecated/archive含めて絞り込みをかけず、rerank_scoreでは
    なくdate降順(新しい順)で並べ替えて返す。過去と現在を見比べたい
    質問(「前はどうだったか」等)向けで、新旧両方を漏らさず時系列で
    並べることを優先する(データ管理法見直し3-1節のrecall重視の方針を踏襲)。
    """
    # 【Ver3.0で変更】検索の都度の全件mtimeスキャン(sync_index)は廃止した。
    # 書き込み時フック(/reindex_file)+起動時1回だけの全件スキャンに切り替えた
    # ため、ここでは単にコレクションを取得するだけでよい(データ管理法見直し
    # 2-5節)。
    collection = _chroma_client.get_or_create_collection(COLLECTION_NAME)
    # クエリのフィラー語除去(正規化)はRust側(text_transformエンジン、
    # prompts/transforms/query-normalization.json)で一元化しているため、
    # ここでは受け取ったクエリをそのまま使う。
    query_embedding = get_embedding(query, client=_http_client, is_query=True)
    # 埋め込みの第一段階だけで足切りすると、言い回しによっては正解チャンクが
    # 候補プールにすら入らないことがある(下記コメント参照)ため、コレクション
    # の実際の総件数を上限として、可能な限り全件をリランカーの判断に委ねる。
    pool_size = min(collection.count(), RERANK_CANDIDATE_POOL_MAX)

    # 現在モードのみstatus: deprecatedを候補プールの時点で除外する
    # (経緯モードはdeprecatedも含めて見比べたいので絞り込まない)。
    combined_where = where
    if mode == "current":
        status_filter = {"status": {"$ne": "deprecated"}}
        combined_where = {"$and": [where, status_filter]} if where else status_filter

    query_kwargs: dict = {"query_embeddings": [query_embedding], "n_results": pool_size}
    if combined_where:
        query_kwargs["where"] = combined_where
    result = collection.query(**query_kwargs)

    ids = result["ids"][0]
    documents = result["documents"][0]
    metadatas = result["metadatas"][0]
    distances = result["distances"][0]

    # n_resultsをコレクションの総件数ぎりぎりまで広げると、ChromaDBが
    # 末尾の枠をmetadata=None等のダミー値で埋めて返すことがある(実測で確認)。
    # そのような不完全な行は候補から除外する。
    valid = [
        (i, d, m, dist)
        for i, d, m, dist in zip(ids, documents, metadatas, distances)
        if m is not None and d is not None
    ]
    ids = [v[0] for v in valid]
    documents = [v[1] for v in valid]
    metadatas = [v[2] for v in valid]
    distances = [v[3] for v in valid]

    # ingest.py側の埋め込み時と同じ「見出し: 本文」形式でリランカーに渡す。
    # 見出しの短い言い回しが、質問文とのマッチングの手がかりになるため。
    passages = [
        f"{meta.get('heading', '')}: {doc}" if meta.get("heading") != "(見出しなし)" else doc
        for doc, meta in zip(documents, metadatas)
    ]
    rerank_scores = rerank(query, passages)

    candidates = list(zip(ids, documents, metadatas, distances, rerank_scores))

    if mode == "history":
        # 経緯モードはrerank_scoreではなくdate降順(新しい順)で並べる。
        # 日付不明の記事は最後尾に回す(datetime.minでの比較)。
        candidates.sort(
            key=lambda c: _parse_date(c[2].get("date", "")) or datetime.min.replace(tzinfo=timezone.utc),
            reverse=True,
        )
        return candidates

    # 現在モードは時間減衰をrerank_scoreに掛け合わせてから並べ替える。
    candidates = [
        (i, d, m, dist, score * _time_decay_factor(m.get("date", "")))
        for i, d, m, dist, score in candidates
    ]
    candidates.sort(key=lambda c: c[4], reverse=True)
    if apply_threshold:
        candidates = [c for c in candidates if c[4] >= RERANK_SCORE_THRESHOLD]
    return candidates


class AddDocumentRequest(BaseModel):
    id: str
    text: str
    heading: str
    source: str
    source_category: str


class ChunkItem(BaseModel):
    id: str
    text: str
    source: str
    heading: str
    source_category: str = "uncategorized"


@app.get("/health")
def health():
    return {"status": "ok"}


@app.post("/add_document")
def add_document(req: AddDocumentRequest):
    """メモ機能(Rust側のmemo_guard)向け。ingest.pyの全件再投入を待たず、
    単一ドキュメントをその場で埋め込み・追加し、直後のpassive recallから
    即座に対象になるようにする。ingest.py側の埋め込み時と同じ「見出し: 本文」
    形式で埋め込む(検索精度を揃えるため)。同じidで再度呼ぶと上書きになる
    (ChromaDBのadd()はid重複時にエラーになるためupsert()を使う)。
    """
    # get_or_create_collection: 検索系(sync_index)と取得方法を揃える。
    # get_collection()のままだと、まだ一度も検索が走らずコレクション未作成の
    # 状態でこのエンドポイントを先に呼んだ場合だけ例外になり、挙動が不揃いに
    # なっていた(2026-08-17、プロジェクト整合性レビューで発覚)。
    collection = _chroma_client.get_or_create_collection(COLLECTION_NAME)
    embed_text = f"{req.heading}: {req.text}" if req.heading and req.heading != "(見出しなし)" else req.text
    embedding = get_embedding(embed_text, client=_http_client, is_query=False)
    collection.upsert(
        ids=[req.id],
        documents=[req.text],
        embeddings=[embedding],
        metadatas=[
            {
                "source": req.source,
                "heading": req.heading,
                "source_category": req.source_category,
            }
        ],
    )
    return {"status": "ok"}


class ReindexFileRequest(BaseModel):
    # KNOWLEDGE_DIR(library/)からの相対パス。
    path: str


@app.post("/reindex_file")
def reindex_file(req: ReindexFileRequest):
    """書き込み時フック(詩織Ver3.0、データ管理法見直し2-5節)向け。shiori-save
    CLI等がlibrary/へ新規保存した直後に呼ばれ、検索の都度の全件スキャンを
    待たず対象ファイル1件だけを即座に埋め込む。呼び出し側(shiori-save CLI)は
    RAGサーバーが起動していない場合はそもそもこのエンドポイントを呼ばない
    設計のため、ここでは通信断のケースは考慮しない(ファイル不在のみ考慮する)。
    """
    try:
        count = reindex_single_file(
            _chroma_client, _http_client, KNOWLEDGE_DIR, VECTORDB_DIR, COLLECTION_NAME, req.path
        )
    except FileNotFoundError:
        return {"status": "not_found", "chunks": 0}
    return {"status": "ok", "chunks": count}


@app.get("/list_all", response_model=list[ChunkItem])
def list_all():
    """スタンドアロン図書館UI(2026-08-12、図書館ビジョン統合仕様書3-2)向け。
    検索ではなく一覧取得のため、embedding計算・リランクは行わずChromaDBの
    全チャンクをそのまま返す。現状の蔵書規模(数十件)なら一括取得で十分
    (件数が大きく増えた場合はページネーションを検討する)。
    """
    collection = _chroma_client.get_or_create_collection(COLLECTION_NAME)
    result = collection.get()

    items: list[ChunkItem] = []
    for chunk_id, doc, meta in zip(result["ids"], result["documents"], result["metadatas"]):
        meta = meta or {}
        items.append(
            ChunkItem(
                id=chunk_id,
                text=doc,
                source=meta.get("source", ""),
                heading=meta.get("heading", ""),
                source_category=meta.get("source_category", "uncategorized"),
            )
        )
    return items


class LibraryFileHeading(BaseModel):
    heading: str


class LibraryFileItem(BaseModel):
    # library/からの相対パス相当(chromadbのmetadata "source"をそのまま使う)。
    source: str
    source_category: str
    # そのファイル内の見出し一覧(登場順、重複なし)。
    headings: list[LibraryFileHeading]
    # frontmatterのtitle(無ければ空文字列。GUI側でファイル名にフォールバック
    # する)。2026-09-16追加、エクスプローラー風UI刷新向け。
    title: str
    # 実ファイルへの絶対パス。search_libraryのpathと同じ_resolve_source_path
    # で解決する(2026-09-16追加)。
    path: str
    # library_rootからの相対パス(/区切り)。GUIがフォルダツリーを構築する際、
    # 絶対パス文字列の解析に頼らず安全に階層を取り出せるようにする
    # (2026-09-16追加)。
    relative_path: str
    # ファイルの更新日時(Unixタイムスタンプ、無ければ0。2026-09-16追加)。
    mtime: float


@app.get("/list_all_library", response_model=list[LibraryFileItem])
def list_all_library():
    """スタンドアロン図書館UI(Phase 7、1冊=1ファイル表示単位への変更)向け。
    /list_allと同じくクエリを伴わない全件一覧のため、embedding計算・リランクは
    行わない。/list_allとの違いはチャンク単位ではなくファイル(source)単位に
    集約して返す点(search_libraryと同じ集約方針だが、検索ではなく一覧なので
    スコアは持たない)。

    【Ver3.0で変更】以前はここで遅延再インデックス(sync_index)を走らせていたが、
    書き込み時フック(/reindex_file)+起動時1回だけの全件スキャンに切り替えた
    ため、ここでは単にコレクションを取得するだけでよい(データ管理法見直し
    2-5節)。
    """
    collection = _chroma_client.get_or_create_collection(COLLECTION_NAME)
    result = collection.get()

    files: dict[str, dict] = {}
    order: list[str] = []
    for doc_meta in result["metadatas"]:
        meta = doc_meta or {}
        source = meta.get("source", "")
        if source not in files:
            files[source] = {
                "source_category": meta.get("source_category", "uncategorized"),
                "headings": [],
                "title": meta.get("title", ""),
                "relative_path": meta.get("relative_path", ""),
                "mtime": meta.get("mtime", 0.0),
            }
            order.append(source)
        heading = meta.get("heading", "")
        existing_headings = files[source]["headings"]
        if heading and heading not in [h.heading for h in existing_headings]:
            existing_headings.append(LibraryFileHeading(heading=heading))

    return [
        LibraryFileItem(
            source=source,
            source_category=files[source]["source_category"],
            headings=files[source]["headings"],
            title=files[source]["title"],
            path=_resolve_source_path(files[source]["source_category"], source),
            relative_path=files[source]["relative_path"],
            mtime=files[source]["mtime"],
        )
        for source in sorted(order)
    ]


@app.post("/search", response_model=list[SearchResultItem])
def search(req: SearchRequest):
    """会話UI向け。チャンク単位・本文込みで返す(LLMへの文脈注入に使うため)。
    識別ガード・メモガード・自発的想起(passive recall)・明示検索
    (search_knowledgeツール)がいずれもこのエンドポイントを使う。挙動は
    Ver2.0でも変更しない(/search_libraryとは別に維持する。下記参照)。
    """
    candidates = _retrieve_and_rerank(req.query, mode=req.mode)[: req.top_k]

    items: list[SearchResultItem] = []
    for chunk_id, doc, meta, dist, score in candidates:
        items.append(
            SearchResultItem(
                id=chunk_id,
                text=doc,
                source=meta.get("source", ""),
                heading=meta.get("heading", ""),
                distance=dist,
                rerank_score=score,
                source_category=meta.get("source_category", "uncategorized"),
            )
        )
    return items


class LibrarySearchFilter(BaseModel):
    author: str | None = None
    type: str | None = None
    project: str | None = None


class SearchLibraryRequest(BaseModel):
    query: str
    # 【Ver3.0で変更】5→20。ページング方式導入に伴うデフォルト件数の見直し
    # (データ管理法見直し3-3節)。
    limit: int = 20
    # ページングの開始位置(ファイル単位の並びに対するオフセット)。
    # 1回目はoffset=0、続きが欲しければoffset=20, 40...と増やして再クエリする。
    offset: int = 0
    filter: LibrarySearchFilter | None = None
    # 詩織Ver3.3(時間認識検索)で新設。"current"(デフォルト)/"history"は
    # _retrieve_and_rerank参照。MCPサーバーのsearch_libraryツールでは
    # include_historyパラメータがこれに変換される。
    mode: Literal["current", "history"] = "current"


class FileHeading(BaseModel):
    heading: str
    rerank_score: float


class SearchLibraryResultItem(BaseModel):
    # library/からの相対パス相当(chromadbのmetadata "source"をそのまま使う)。
    source: str
    source_category: str
    # そのファイル内でヒットした見出しのリスト(スコア降順)。
    headings: list[FileHeading]
    # ファイル自体の並び順に使う、ファイル内最高スコア。
    best_score: float
    # 実ファイルへの絶対パス(見つからなければ空文字列)。chromadbのmetadataは
    # ファイル名(source)とトップレベルフォルダ(source_category)しか
    # 持たず、中間のサブディレクトリ(project識別子等)が分からないため、
    # source_category配下を都度rglobして解決する(2026-09-16追加)。
    path: str
    # frontmatterのtitle(無ければ空文字列)。2026-09-16追加。
    title: str
    # frontmatterのrelated(無ければ空リスト)。詩織Ver3.3(時間認識検索)で
    # 追加。経緯モードで新旧の記事を辿るための手がかり。
    related: list[str] = []


def _resolve_source_path(source_category: str, source: str) -> str:
    """source_category配下から、ファイル名(source)が一致する実ファイルの
    絶対パスを探す。sourceはlibrary全体で一意という既存の前提(indexing.pyが
    metadataのキーとしてファイル名のみを使っている)を踏襲する。
    """
    search_root = KNOWLEDGE_DIR / source_category
    if not search_root.is_dir():
        return ""
    match = next(search_root.rglob(source), None)
    return str(match) if match else ""


@app.post("/search_library", response_model=list[SearchLibraryResultItem])
def search_library(req: SearchLibraryRequest):
    """MCPサーバーのsearch_libraryツール向け(詩織Ver2.0設計指示書v3、9章)。
    /searchと違い、結果をファイル単位に集約し、本文は含めない(司書は棚の
    場所(見出し)を教えるだけで、中身を合成しない、という設計方針)。1冊=
    1ファイルという表示単位(10章)とも整合させている。ただし実ファイルへの
    絶対パス(path)は返すため、呼び出し側(Claude Code)はそれを使って
    Readツール等で直接本文を読める(2026-09-16追加。棚の場所を教えるだけ、
    という設計方針は変えず、「棚の正確な位置(パス)まで教える」形に強化した)。

    filterのauthor/type/projectは、Ver2.0の保存ルーティング(shiori-save CLI)が
    chromadbのメタデータに実際に書き込むフィールドで絞り込む(authorは
    Ver3.0で書き込み自体を廃止したため、現行データでは該当なしになる)。

    【Ver3.0で変更】RERANK_SCORE_THRESHOLDによる足切りを行わず(recall重視の
    方針、_retrieve_and_rerank参照)、offset+limitのページング方式で返す件数を
    制御する(データ管理法見直し3-3節)。呼び出し側(Claude Code)は1回目
    offset=0で呼び、欲しい情報が見つからなければoffset=20,40...と増やして
    再クエリする。候補プール(最大200件)の終端に達しlimit件に満たない結果が
    返れば「これ以上情報がない」という終了シグナルになる。
    """
    where: dict | None = None
    if req.filter:
        conditions = []
        if req.filter.author:
            conditions.append({"author": req.filter.author})
        if req.filter.type:
            conditions.append({"type": req.filter.type})
        if req.filter.project:
            conditions.append({"project": req.filter.project})
        if len(conditions) == 1:
            where = conditions[0]
        elif len(conditions) > 1:
            where = {"$and": conditions}

    candidates = _retrieve_and_rerank(req.query, where, apply_threshold=False, mode=req.mode)

    # ファイル単位(source)に集約する。candidatesは既にスコア降順
    # (経緯モードではdate降順)のため、各ファイルの初出順がそのまま
    # ファイルの並び順になる。
    files: dict[str, dict] = {}
    order: list[str] = []
    for _chunk_id, _doc, meta, _dist, score in candidates:
        source = meta.get("source", "")
        if source not in files:
            related_raw = meta.get("related", "")
            files[source] = {
                "source_category": meta.get("source_category", "uncategorized"),
                "headings": [],
                "best_score": score,
                "title": meta.get("title", ""),
                "related": related_raw.split("|") if related_raw else [],
            }
            order.append(source)
        files[source]["headings"].append(FileHeading(heading=meta.get("heading", ""), rerank_score=score))

    items = [
        SearchLibraryResultItem(
            source=source,
            source_category=files[source]["source_category"],
            headings=files[source]["headings"],
            best_score=files[source]["best_score"],
            path=_resolve_source_path(files[source]["source_category"], source),
            title=files[source]["title"],
            related=files[source]["related"],
        )
        for source in order
    ]
    return items[req.offset : req.offset + req.limit]
