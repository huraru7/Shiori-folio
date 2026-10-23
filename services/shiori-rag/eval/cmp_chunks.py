"""py_chunks.py と shiori-rag dump-chunks の出力を比べ、チャンクの違いを数える。
使い方: python cmp_chunks.py <Python版のJSONL> <Rust版のJSONL>
"""
import json, sys
py = {j["rel"]: j for j in map(json.loads, open(sys.argv[1], encoding="utf-8"))}
rs = {j["rel"]: j for j in map(json.loads, open(sys.argv[2], encoding="utf-8"))}
print("files", len(py), len(rs), "only_py", set(py)-set(rs), "only_rs", set(rs)-set(py))
n_chunks = sum(len(j["chunks"]) for j in py.values())
diff_files = 0; diff_chunks = 0
for rel in sorted(py):
    a, b = py[rel]["chunks"], rs.get(rel, {}).get("chunks", [])
    if a != b:
        diff_files += 1
        if len(a) != len(b):
            print("count", rel, len(a), len(b)); continue
        for x, y in zip(a, b):
            if x != y:
                diff_chunks += 1
                if diff_chunks <= 5:
                    import difflib
                    print("DIFF", rel, x["heading"] == y["heading"], len(x["text"]), len(y["text"]))
                    for i,(c1,c2) in enumerate(zip(x["text"],y["text"])):
                        if c1!=c2: print("  at", i, repr(x["text"][max(0,i-20):i+20]), repr(y["text"][max(0,i-20):i+20])); break
print("total chunks", n_chunks, "diff files", diff_files, "diff chunks", diff_chunks)
