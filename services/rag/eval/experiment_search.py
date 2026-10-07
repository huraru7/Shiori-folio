"""検索品質(Ver3.8)の候補比較実験。本番のChromaDBには一切触れない。

起動中のRAGサーバーの/list_allで全チャンクを取り、起動中の埋め込みサーバー(config.jsonの
embedding.port)とリランカー(reranker.py)だけを使って、検索パイプラインの変種をメモリ上で再現し、
eval/search_dataset.jsonの質問に対する指標を比較する。

実行(rag-venvのpythonで、services/rag/で): python eval/experiment_search.py [--variants base,a1,...]

変種:
  base  現行の再現: 密ベクトル上位200件 → リランク → ファイル単位に集約
  a2    base+リランク入力に「記事のタイトル+要約」を前置(埋め込みは再計算しない)
  a1    base+埋め込みもリランクも「記事のタイトル+要約」を前置(全チャンクを埋め込み直す)
  b     base+語彙一致(文字bigramのBM25)の上位も候補に加えてからリランク
  ab    a1+b
注意: 時間減衰(現在モード)と足切りは再現していない(日付がlist_allに無いため)。
"""
from __future__ import annotations

import argparse
import json
import math
import re
import sys
import unicodedata
import urllib.request
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from reranker import rerank  # noqa: E402

CONFIG = json.loads((HERE.parents[2] / "config.json").read_text(encoding="utf-8"))
EMBED_URL = f"http://127.0.0.1:{CONFIG['embedding']['port']}/v1/embeddings"
RAG_URL = f"http://127.0.0.1:{CONFIG['rag']['port']}"
POOL = 200
CACHE = Path("/tmp/shiori-exp-cache") if sys.platform != "win32" else Path.home() / "shiori-exp-cache"


def nfc(s: str) -> str:
    return unicodedata.normalize("NFC", s)


def http_json(url: str, body: dict | None = None):
    data = json.dumps(body).encode("utf-8") if body is not None else None
    req = urllib.request.Request(url, data=data, headers={"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.load(r)


def embed(text: str) -> list[float]:
    return http_json(EMBED_URL, {"input": text})["data"][0]["embedding"]


def embed_many(texts: list[str], threads: int = 4) -> list[list[float]]:
    with ThreadPoolExecutor(threads) as ex:
        return list(ex.map(embed, texts))


def cosine_rank(qv: list[float], vecs: list[list[float]]) -> list[int]:
    import numpy as np

    m = np.array(vecs)
    q = np.array(qv)
    sims = (m @ q) / (np.linalg.norm(m, axis=1) * np.linalg.norm(q) + 1e-9)
    return list(np.argsort(-sims))


def bigrams(s: str) -> list[str]:
    s = re.sub(r"\s+", "", nfc(s).lower())
    return [s[i : i + 2] for i in range(len(s) - 1)] or [s]


class Bm25:
    def __init__(self, docs: list[str], k1: float = 1.5, b: float = 0.75):
        self.tf = [Counter(bigrams(d)) for d in docs]
        self.len = [sum(t.values()) for t in self.tf]
        self.avg = sum(self.len) / len(self.len)
        df = Counter()
        for t in self.tf:
            df.update(t.keys())
        n = len(docs)
        self.idf = {w: math.log(1 + (n - c + 0.5) / (c + 0.5)) for w, c in df.items()}
        self.k1, self.b = k1, b

    def scores(self, query: str) -> list[float]:
        qs = bigrams(query)
        out = []
        for tf, ln in zip(self.tf, self.len):
            s = 0.0
            for w in qs:
                f = tf.get(w, 0)
                if f:
                    s += self.idf.get(w, 0) * f * (self.k1 + 1) / (f + self.k1 * (1 - self.b + self.b * ln / self.avg))
            out.append(s)
        return out


def load_doc_info(library: Path) -> dict[str, tuple[str, str]]:
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


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--variants", default="base,a2,a1,b,ab")
    # 実機のlibrary(開発ツリーの`Shiori-folio/library/`は空のコピーなので使わない)。
    ap.add_argument("--library", default="/Volumes/ShioriFolio/portable/library")
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    dataset = json.load(open(HERE / "search_dataset.json", encoding="utf-8"))
    global_ignore = {nfc(x) for x in dataset.get("ignore_sources", [])}
    items = [it for it in dataset["items"] if it["kind"] != "not_found"]

    chunks = http_json(RAG_URL + "/list_all")
    info = load_doc_info(Path(args.library))
    print(f"chunks={len(chunks)} queries={len(items)} docs_with_info={len(info)}", flush=True)

    def plain(c: dict) -> str:
        return f"{c['heading']}: {c['text']}" if c["heading"] != "(見出しなし)" else c["text"]

    def with_ctx(c: dict) -> str:
        title, summary = info.get(nfc(c["source"]), ("", ""))
        head = "。".join(x for x in (title, summary) if x)
        return f"{head}\n{plain(c)}" if head else plain(c)

    CACHE.mkdir(parents=True, exist_ok=True)

    def embeddings(kind: str, texts: list[str]) -> list[list[float]]:
        f = CACHE / f"emb_{kind}_{len(texts)}.json"
        if f.exists():
            return json.load(open(f))
        print(f"埋め込み計算: {kind} ({len(texts)}件)", flush=True)
        v = embed_many(["search_document: " + t for t in texts])
        json.dump(v, open(f, "w"))
        return v

    plain_texts = [plain(c) for c in chunks]
    ctx_texts = [with_ctx(c) for c in chunks]
    qvecs = {it["id"]: embed("search_query: " + it["query"]) for it in items}

    results = {}
    for variant in args.variants.split(","):
        use_ctx_embed = variant in ("a1", "ab")
        use_ctx_rerank = variant in ("a1", "a2", "ab")
        use_bm25 = variant in ("b", "ab")
        vecs = embeddings("ctx" if use_ctx_embed else "plain", ctx_texts if use_ctx_embed else plain_texts)
        bm = Bm25(ctx_texts if use_ctx_embed else plain_texts) if use_bm25 else None
        passages = ctx_texts if use_ctx_rerank else plain_texts

        rows = []
        for it in items:
            order = cosine_rank(qvecs[it["id"]], vecs)[:POOL]
            cand = list(order)
            if bm is not None:
                s = bm.scores(it["query"])
                top = sorted(range(len(s)), key=lambda i: -s[i])[:POOL // 2]
                cand = list(dict.fromkeys(cand + top))
            scores = rerank(it["query"], [passages[i] for i in cand])
            best: dict[str, float] = {}
            for i, sc in zip(cand, scores):
                src = nfc(chunks[i]["source"])
                if src in global_ignore or src in {nfc(x) for x in it.get("ignore", [])}:
                    continue
                best[src] = max(best.get(src, 0.0), sc)
            ranked = sorted(best, key=lambda k: -best[k])
            expected = {nfc(x) for x in it["expected"]}
            rank = next((i + 1 for i, s2 in enumerate(ranked) if s2 in expected), None)
            rows.append((it, rank, ranked[:3]))
        n = len(rows)
        summ = {
            "hit@1": sum(1 for _, r, _ in rows if r == 1) / n,
            "hit@3": sum(1 for _, r, _ in rows if r and r <= 3) / n,
            "hit@5": sum(1 for _, r, _ in rows if r and r <= 5) / n,
            "mrr": sum(1 / r for _, r, _ in rows if r) / n,
        }
        results[variant] = {"summary": summ, "ranks": {it["id"]: r for it, r, _ in rows}}
        print(f"[{variant}] " + " ".join(f"{k}={v:.3f}" for k, v in summ.items()), flush=True)
        if args.verbose:
            for it, r, top in rows:
                if r is None or r > 5:
                    print(f"    NG {it['id']} rank={r} {it['query']} -> {top}")

    print("\n質問別の順位(None=20位圏外):")
    ids = [it["id"] for it in items]
    vs = list(results)
    print("id   " + " ".join(f"{v:>5}" for v in vs))
    for i in ids:
        print(f"{i}  " + " ".join(f"{str(results[v]['ranks'][i]):>5}" for v in vs))
    out = HERE / "results" / "experiment_search_latest.json"
    out.write_text(json.dumps(results, ensure_ascii=False, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
