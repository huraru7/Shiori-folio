"""RAG検索サーバー。/search でクエリを受け取り、embedding-server経由で
ベクトル化してChromaDBのshiori_knowledgeコレクションをtop_k件検索する。

起動: .venv\\Scripts\\uvicorn.exe app:app --port 8083 --host 127.0.0.1
"""
from __future__ import annotations

from pathlib import Path

import chromadb
import httpx
from fastapi import FastAPI
from pydantic import BaseModel

from embedding_client import get_embedding
from reranker import rerank

PROJECT_ROOT = Path(__file__).resolve().parents[2]
VECTORDB_DIR = PROJECT_ROOT / "data" / "vectordb"
COLLECTION_NAME = "shiori_knowledge"

app = FastAPI()
_chroma_client = chromadb.PersistentClient(path=str(VECTORDB_DIR))
_http_client = httpx.Client(timeout=30.0)


class SearchRequest(BaseModel):
    query: str
    top_k: int = 5


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
    collection = _chroma_client.get_collection(COLLECTION_NAME)
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


@app.get("/list_all", response_model=list[ChunkItem])
def list_all():
    """スタンドアロン図書館UI(2026-08-12、図書館ビジョン統合仕様書3-2)向け。
    検索ではなく一覧取得のため、embedding計算・リランクは行わずChromaDBの
    全チャンクをそのまま返す。現状の蔵書規模(数十件)なら一括取得で十分
    (件数が大きく増えた場合はページネーションを検討する)。
    """
    collection = _chroma_client.get_collection(COLLECTION_NAME)
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


@app.post("/search", response_model=list[SearchResultItem])
def search(req: SearchRequest):
    collection = _chroma_client.get_collection(COLLECTION_NAME)
    # クエリのフィラー語除去(正規化)はRust側(text_transformエンジン、
    # prompts/transforms/query-normalization.json)で一元化しているため、
    # ここでは受け取ったクエリをそのまま使う。
    query_embedding = get_embedding(req.query, client=_http_client, is_query=True)
    # 埋め込みの第一段階だけで足切りすると、言い回しによっては正解チャンクが
    # 候補プールにすら入らないことがある(上記コメント参照)ため、コレクション
    # の総件数を上限として、可能な限り全件をリランカーの判断に委ねる。
    pool_size = min(collection.count(), RERANK_CANDIDATE_POOL_MAX)
    pool_size = max(req.top_k, pool_size)
    result = collection.query(query_embeddings=[query_embedding], n_results=pool_size)

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
    rerank_scores = rerank(req.query, passages)

    candidates = list(zip(ids, documents, metadatas, distances, rerank_scores))
    candidates.sort(key=lambda c: c[4], reverse=True)
    candidates = [c for c in candidates if c[4] >= RERANK_SCORE_THRESHOLD]
    candidates = candidates[: req.top_k]

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
