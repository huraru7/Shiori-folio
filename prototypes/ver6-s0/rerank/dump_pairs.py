"""Ver6.0 段階0の試作1: リランカーの比較用に、質問と文章の組とPython版の点数を書き出す。

いまのRAG(services/rag)の部品だけを読み込み、RAGのコードと本番のChromaDBには触れない。
起動中のRAGサーバーの/list_allで全チャンクを取り、記事のタイトルと要約をlibraryのfrontmatterから
足して、本番と同じ形の文章(passage.pyのpassage_text)を作る。候補の選び方は本番の再現ではなく、
語句の一致(BM25)の上位と、無作為に選んだもの(点数の低い組も比べるため)を混ぜる。

実行(rag-venvのpythonで、このフォルダで):
    python dump_pairs.py [--library E:/portable/library] [--queries 20]
出力: data/pairs.json(記事の本文を含むので、gitには入れない)
"""
from __future__ import annotations

import argparse
import json
import math
import os
import random
import re
import sys
import time
import unicodedata
import urllib.request
from pathlib import Path

os.environ.setdefault("HF_HUB_OFFLINE", "1")

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
RAG_DIR = REPO / "services" / "rag"
sys.path.insert(0, str(RAG_DIR))
sys.path.insert(0, str(RAG_DIR / "eval"))

import torch  # noqa: E402
from experiment_search import Bm25  # noqa: E402
from passage import passage_text  # noqa: E402
from reranker import _get_model, rerank  # noqa: E402

CONFIG = json.loads((REPO / "config.json").read_text(encoding="utf-8"))
RAG_URL = f"http://127.0.0.1:{CONFIG['rag']['port']}"

# 比較用の候補: BM25の上位と、無作為の分
TOP_BM25 = 40
RANDOM_EXTRA = 10
# 速さの比較用: 本番の候補の上限(RERANK_CANDIDATE_POOL_MAX)と同じ件数
TIMING_QUERIES = 3
TIMING_POOL = 200
# トークンのIDを書き出す組の数(質問ごと)
TOKEN_SAMPLES = 3


def nfc(s: str) -> str:
    return unicodedata.normalize("NFC", s)


def http_json(url: str):
    with urllib.request.urlopen(url, timeout=120) as r:
        return json.load(r)


def load_doc_info(library: Path) -> dict[str, tuple[str, str]]:
    """ファイル名→(title, summary)。experiment_search.pyと同じ読み方。"""
    info: dict[str, tuple[str, str]] = {}
    for p in library.rglob("*.md"):
        if p.name.startswith("._"):
            continue
        t = p.read_text(encoding="utf-8")
        m = re.match(r"---\n(.*?)\n---", t, re.S)
        fm = m.group(1) if m else ""

        def g(k: str) -> str:
            mm = re.search(rf"^{k}:\s*(.*)$", fm, re.M)
            return mm.group(1).strip().strip("\"'") if mm else ""

        info[nfc(p.name)] = (g("title"), g("summary"))
    return info


def pick_queries(items: list[dict], n: int) -> list[dict]:
    """kindごとに順番に1問ずつ取り、種類を散らす。"""
    by_kind: dict[str, list[dict]] = {}
    for it in items:
        by_kind.setdefault(it["kind"], []).append(it)
    picked: list[dict] = []
    while len(picked) < n and any(by_kind.values()):
        for kind in sorted(by_kind):
            if by_kind[kind] and len(picked) < n:
                picked.append(by_kind[kind].pop(0))
    return picked


def timed_rerank(query: str, passages: list[str]) -> tuple[list[float], float]:
    start = time.perf_counter()
    scores = rerank(query, passages)
    return scores, time.perf_counter() - start


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--library", default="E:/portable/library")
    ap.add_argument("--queries", type=int, default=20)
    args = ap.parse_args()

    dataset = json.load(open(RAG_DIR / "eval" / "search_dataset.json", encoding="utf-8"))
    items = [it for it in dataset["items"] if it["kind"] != "not_found"]
    queries = pick_queries(items, args.queries)

    chunks = http_json(RAG_URL + "/list_all")
    info = load_doc_info(Path(args.library))
    passages = []
    for c in chunks:
        title, summary = info.get(nfc(c["source"]), ("", ""))
        passages.append(passage_text(title, summary, c["heading"], c["text"]))
    print(f"chunks={len(chunks)} queries={len(queries)}", flush=True)

    bm25 = Bm25(passages)
    model = _get_model()
    tokenizer = model.tokenizer

    # 初回の呼び出しの遅れを測定から外すため、1回だけ空回しする
    rerank("準備", passages[:4])

    out_queries = []
    for qi, it in enumerate(queries):
        query = it["query"]
        scores_bm = bm25.scores(query)
        order = sorted(range(len(passages)), key=lambda i: -scores_bm[i])
        cand = order[:TOP_BM25]
        rest = order[TOP_BM25:]
        rng = random.Random(it["id"])
        cand += rng.sample(rest, min(RANDOM_EXTRA, len(rest)))
        texts = [passages[i] for i in cand]

        # 本番と同じ関数(シグモイド後)と、生の値(活性化なし)の両方を取る
        scores, elapsed = timed_rerank(query, texts)
        raw = model.predict([(query, t) for t in texts], activation_fn=torch.nn.Identity())
        raw = [float(x) for x in raw]
        sig_check = max(abs(1 / (1 + math.exp(-r)) - s) for r, s in zip(raw, scores))

        token_ids = []
        for t in texts[:TOKEN_SAMPLES]:
            enc = tokenizer(query, t, truncation=True, max_length=model.max_length)
            token_ids.append(enc["input_ids"])

        out_queries.append({
            "id": it["id"],
            "kind": it["kind"],
            "query": query,
            "passages": texts,
            "chunk_ids": [chunks[i]["id"] for i in cand],
            "py_scores": scores,
            "py_raw": raw,
            "py_seconds": elapsed,
            "token_ids": token_ids,
        })
        print(f"{it['id']} n={len(texts)} {elapsed:.2f}s sigmoid_check={sig_check:.2e}", flush=True)

    timing = []
    for it in queries[:TIMING_QUERIES]:
        query = it["query"]
        scores_bm = bm25.scores(query)
        order = sorted(range(len(passages)), key=lambda i: -scores_bm[i])[:TIMING_POOL]
        texts = [passages[i] for i in order]
        scores, elapsed = timed_rerank(query, texts)
        timing.append({"id": it["id"], "query": query, "passages": texts, "py_scores": scores, "py_seconds": elapsed})
        print(f"timing {it['id']} n={len(texts)} {elapsed:.2f}s", flush=True)

    data_dir = HERE / "data"
    data_dir.mkdir(exist_ok=True)
    meta = {
        "model": "hotchpotch/japanese-reranker-xsmall-v2",
        "torch_threads": torch.get_num_threads(),
        "max_length": model.max_length,
        "activation": str(model.activation_fn),
    }
    (data_dir / "pairs.json").write_text(
        json.dumps({"meta": meta, "queries": out_queries, "timing": timing}, ensure_ascii=False),
        encoding="utf-8",
    )
    print("wrote", data_dir / "pairs.json")


if __name__ == "__main__":
    main()
