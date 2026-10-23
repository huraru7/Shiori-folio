"""Python版とRust版の評価の結果(run_search_eval.pyが書いたJSON)を、質問ごとに並べる。
使い方: python cmp_eval.py <python版のJSON> <rust版のJSON>
"""
import json
import sys

a = json.load(open(sys.argv[1], encoding="utf-8"))
b = json.load(open(sys.argv[2], encoding="utf-8"))
print("| 質問 | 種類 | Python版の順位 | Rust版の順位 | 上位3件が同じ |")
print("|---|---|---|---|---|")
same_top = 0
for x, y in zip(a["rows"], b["rows"]):
    assert x["id"] == y["id"]
    s = x["top"] == y["top"]
    same_top += s
    if x["rank"] != y["rank"] or not s:
        print(f"| {x['id']} | {x['kind']} | {x['rank']} | {y['rank']} | {'同じ' if s else '違う'} |")
print()
print(f"上位3件が同じ: {same_top}/{len(a['rows'])}")
for k in ["hit@1", "hit@3", "hit@5", "mrr"]:
    print(k, round(a["summary"]["overall"][k], 3), round(b["summary"]["overall"][k], 3))
print("not_found", a["summary"]["not_found_correct"], b["summary"]["not_found_correct"])
