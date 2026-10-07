"""検索品質(Ver3.8)の評価スクリプト。起動中のRAGサーバーのHTTP APIだけを使う(標準ライブラリのみ)。

実行: python3 eval/run_search_eval.py --tag baseline [--base http://127.0.0.1:8083] [--verbose]

- 通常の質問は/search_libraryを呼び、ファイル単位の並びで最初に当たった正解の順位を測る。
  指標はhit@1/3/5とMRR(正解の順位の逆数の平均)。kind別にも集計する。
- not_foundの質問は/search(会話UI向け、足切りあり)を呼び、0件なら正答とする。
- 結果はeval/results/search_<timestamp>_<tag>.jsonに保存し、history_search.csvへ1行追記する。
"""
from __future__ import annotations

import argparse
import csv
import json
import unicodedata
import urllib.request
from collections import defaultdict
from datetime import datetime
from pathlib import Path

HERE = Path(__file__).resolve().parent


def post(base: str, path: str, body: dict) -> list[dict]:
    req = urllib.request.Request(
        base + path,
        data=json.dumps(body).encode("utf-8"),
        headers={"content-type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=120) as resp:
        return json.load(resp)


def nfc(s: str) -> str:
    # macOSのファイル名はNFD(濁点が分解された形)で返ることがあるため、比較は常にNFCに揃える。
    return unicodedata.normalize("NFC", s)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tag", required=True)
    ap.add_argument("--base", default="http://127.0.0.1:8083")
    ap.add_argument("--dataset", default=str(HERE / "search_dataset.json"))
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    dataset = json.load(open(args.dataset, encoding="utf-8"))
    items = dataset["items"]
    # 質問文を引用している記事(企画書など)が検索結果の順位を汚すため、順位の計算から除く。
    global_ignore = {nfc(x) for x in dataset.get("ignore_sources", [])}
    rows = []
    for it in items:
        if it["kind"] == "not_found":
            hits = post(args.base, "/search", {"query": it["query"], "top_k": 5})
            rows.append({**it, "rank": None, "correct": len(hits) == 0, "top": [h["source"] for h in hits[:3]]})
            continue
        res = post(args.base, "/search_library", {"query": it["query"], "limit": 20})
        ignore = global_ignore | {nfc(x) for x in it.get("ignore", [])}
        sources = [nfc(r["source"]) for r in res if nfc(r["source"]) not in ignore]
        expected = {nfc(x) for x in it["expected"]}
        rank = next((i + 1 for i, s in enumerate(sources) if s in expected), None)
        rows.append({**it, "rank": rank, "correct": rank is not None and rank <= 5, "top": sources[:3]})

    def summarize(subset: list[dict]) -> dict:
        n = len(subset)
        if n == 0:
            return {"n": 0}
        return {
            "n": n,
            "hit@1": sum(1 for r in subset if r["rank"] == 1) / n,
            "hit@3": sum(1 for r in subset if r["rank"] and r["rank"] <= 3) / n,
            "hit@5": sum(1 for r in subset if r["rank"] and r["rank"] <= 5) / n,
            "mrr": sum(1 / r["rank"] for r in subset if r["rank"]) / n,
        }

    searchable = [r for r in rows if r["kind"] != "not_found"]
    by_kind = defaultdict(list)
    for r in searchable:
        by_kind[r["kind"]].append(r)
    not_found = [r for r in rows if r["kind"] == "not_found"]
    summary = {
        "overall": summarize(searchable),
        "by_kind": {k: summarize(v) for k, v in by_kind.items()},
        "not_found_correct": (sum(1 for r in not_found if r["correct"]) / len(not_found)) if not_found else None,
    }

    if args.verbose:
        for r in rows:
            mark = "OK " if r["correct"] else "NG "
            print(f"{mark}{r['id']} [{r['kind']}] rank={r['rank']} {r['query']}  -> {r['top']}")
    print(json.dumps(summary, ensure_ascii=False, indent=1))

    ts = datetime.now().strftime("%Y%m%d_%H%M%S")
    out = HERE / "results" / f"search_{ts}_{args.tag}.json"
    out.write_text(json.dumps({"tag": args.tag, "summary": summary, "rows": rows}, ensure_ascii=False, indent=1), encoding="utf-8")
    hist = HERE / "results" / "history_search.csv"
    new = not hist.exists()
    with hist.open("a", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        if new:
            w.writerow(["tag", "timestamp", "n", "hit@1", "hit@3", "hit@5", "mrr", "not_found_correct"])
        o = summary["overall"]
        w.writerow([args.tag, ts, o["n"], round(o["hit@1"], 3), round(o["hit@3"], 3), round(o["hit@5"], 3), round(o["mrr"], 3), summary["not_found_correct"]])


if __name__ == "__main__":
    main()
