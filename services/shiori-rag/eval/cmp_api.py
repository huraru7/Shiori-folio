"""同じ要求をPython版とRust版に送り、応答の項目・型・値(距離と点数)と時間を比べる。
使い方: python cmp_api.py <python版のURL> <rust版のURL>
"""
import json
import sys
import time
import urllib.request

PY, RS = sys.argv[1], sys.argv[2]


def call(base, path, body=None):
    t = time.perf_counter()
    if body is None:
        req = urllib.request.Request(base + path)
    else:
        req = urllib.request.Request(base + path, data=json.dumps(body).encode(), headers={"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=300) as r:
        return json.load(r), time.perf_counter() - t


def shape(v):
    if isinstance(v, dict):
        return {k: shape(x) for k, x in v.items()}
    if isinstance(v, list):
        return [shape(v[0])] if v else []
    return type(v).__name__ if not isinstance(v, (int, float)) or isinstance(v, bool) else "number"


cases = [
    ("/search", {"query": "詩織の名前の由来", "top_k": 10}),
    ("/search", {"query": "前はどうだった? 検索の仕組み", "top_k": 5, "mode": "history"}),
    ("/search_library", {"query": "shiori の今の状態", "limit": 5}),
    ("/search_library", {"query": "起動が遅い", "limit": 5, "filter": {"project": "shiori", "exclude_stale": True}}),
    ("/search_library", {"query": "ver3.8", "limit": 5, "mode": "history"}),
    ("/search_library", {"query": "企画書", "limit": 5, "filter": {"kind": "resource", "source_categories": ["20-areas"]}}),
    ("/search_shelf", {"query": "学んだこと", "limit": 3}),
]
for path, body in cases:
    pa, ta = call(PY, path, body)
    rb, tb = call(RS, path, body)
    print(f"== {path} {json.dumps(body, ensure_ascii=False)}  Python {ta:.2f}秒 / Rust {tb:.2f}秒")
    print("  形が同じ:", shape(pa) == shape(rb) if pa and rb else (len(pa), len(rb)))
    key = "id" if path == "/search" else "source"
    print("  Python:", [x[key] for x in pa])
    print("  Rust  :", [x[key] for x in rb])
    if path == "/search":
        for x in pa[:3]:
            y = next((y for y in rb if y["id"] == x["id"]), None)
            if y:
                print(f"   {x['id']}: 距離 {x['distance']:.4f} / {y['distance']:.4f}  点数 {x['rerank_score']:.4f} / {y['rerank_score']:.4f}")
    else:
        for x in pa[:3]:
            y = next((y for y in rb if y["source"] == x["source"]), None)
            if y:
                print(f"   {x['source']}: best {x['best_score']:.4f} / {y['best_score']:.4f}  path同じ {x['path'] == y['path']}")
for path in ["/list_all_library", "/list_all", "/extract_warnings"]:
    pa, ta = call(PY, path)
    rb, tb = call(RS, path)
    if path == "/list_all_library":
        print(f"== {path} 件数 {len(pa)} / {len(rb)}  形が同じ {shape(pa) == shape(rb)}")
        pm = {x["source"]: x for x in pa}
        diffs = [x["source"] for x in rb if x["source"] in pm and {k: v for k, v in pm[x["source"]].items() if k != "mtime"} != {k: v for k, v in x.items() if k != "mtime"}]
        print("  中身が違う(mtime以外):", len(diffs), diffs[:5])
        print("  片方だけ:", sorted(set(pm) ^ {x["source"] for x in rb})[:10])
    elif path == "/list_all":
        print(f"== {path} 件数 {len(pa)} / {len(rb)}  形が同じ {shape(pa) == shape(rb)}")
        pm = {x["id"]: x for x in pa}
        print("  中身が違う:", sum(1 for x in rb if x["id"] in pm and pm[x["id"]] != x), " 片方だけ:", sorted(set(pm) ^ {x["id"] for x in rb})[:10])
    else:
        print(f"== {path}", json.dumps(pa, ensure_ascii=False)[:300], "/", json.dumps(rb, ensure_ascii=False)[:300])
