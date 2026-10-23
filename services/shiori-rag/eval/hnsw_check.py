"""ChromaDBの近似の索引(HNSW)が、厳密な近い順の上位200件をどれだけ取りこぼすかを数える。
実機の索引には触らず、コピーに対して使う。
使い方: rag-venvのpython hnsw_check.py <vectordbのコピー> <質問>
"""
import json, sys, urllib.request
import numpy as np, chromadb
q = sys.argv[2]
emb = json.load(urllib.request.urlopen(urllib.request.Request("http://127.0.0.1:8082/v1/embeddings", data=json.dumps({"input": "search_query: " + q}).encode(), headers={"content-type": "application/json"})))["data"][0]["embedding"]
c = chromadb.PersistentClient(path=sys.argv[1]).get_collection("shiori_knowledge")
where = {"status": {"$ne": "deprecated"}}
r = c.query(query_embeddings=[emb], n_results=200, where=where, include=["metadatas", "distances"])
hnsw_ids = r["ids"][0]
allr = c.get(where=where, include=["embeddings", "metadatas"])
E = np.array(allr["embeddings"], dtype=np.float32); qv = np.array(emb, dtype=np.float32)
d = ((E - qv) ** 2).sum(1)
order = np.argsort(d)
exact_ids = [allr["ids"][i] for i in order[:200]]
print("HNSWの200件と厳密な200件の重なり:", len(set(hnsw_ids) & set(exact_ids)))
missed = [i for i in exact_ids if i not in hnsw_ids]
print("HNSWが取りこぼした(厳密では上位200件):", len(missed))
for i in missed:
    k = allr["ids"].index(i); print("  ", i, allr["metadatas"][k]["source"], "厳密の順位", exact_ids.index(i) + 1)
print("距離の比較(HNSWの上位3件):", [(i, round(x, 4), round(float(d[allr["ids"].index(i)]), 4)) for i, x in zip(hnsw_ids[:3], r["distances"][0][:3])])
