"""RAG検索パイプラインの評価スクリプト(RAGAS、非LLM版Context Precision/Recall)。

実行(services/rag/ディレクトリで、RAGサーバー(/search、ポート8083)と
embedding-server(ポート8082)が起動している状態で):
    .venv\\Scripts\\python.exe eval\\run_eval.py --tag baseline
    .venv\\Scripts\\python.exe eval\\run_eval.py --tag with_reranker

--tagは施策のBefore/After比較用のラベル(任意の文字列)。
結果は eval/results/<timestamp>_<tag>.json に保存され、
eval/results/history.csv に1行追記される(スコアの推移を一覧で見るため)。

指標:
- Context Precision (NonLLMContextPrecisionWithReference): 検索結果の上位に
  ノイズが少ないか。retrieved_contextsの各チャンクをreference_contextsと
  文字列類似度で比較し、閾値以上を「正解」として順位を考慮した精度を計算する
- Context Recall (NonLLMContextRecall): 正解チャンク(reference_contexts)が
  検索結果に含まれているか

注意: retrieved_contexts/reference_contextsはチャンクの本文テキストを渡す
(RAGASのSingleTurnSampleにはretrieved_context_ids/reference_context_idsという
フィールドもあるが、NonLLMContextPrecisionWithReference/NonLLMContextRecallの
実装は本文の文字列類似度で判定する仕様のため、テキストを使う)。
chunk IDが決定的であることを利用して、dataset.json側は本文を持たず
IDだけを持ち、評価実行時にChromaDBから本文を取得することで、
ドキュメント本体の言い回しが変わってもdataset.jsonを直す必要がないようにしている。

expected_contextが空配列の項目(「存在しない情報」を聞くケース)は、
Recallは常に1.0(正解が0件なので満たすものもない)になり評価対象として
機能しないため、代わりに「検索結果が実際に空だったか」を別途
`not_found_correct`として集計する(ハルシネーション検知用)。
"""
from __future__ import annotations

import argparse
import csv
import json
import re
import warnings
from datetime import datetime
from pathlib import Path

import chromadb
import httpx

warnings.filterwarnings("ignore")

from ragas import SingleTurnSample  # noqa: E402
from ragas.metrics import NonLLMContextPrecisionWithReference, NonLLMContextRecall  # noqa: E402

PROJECT_ROOT = Path(__file__).resolve().parents[3]
EVAL_DIR = Path(__file__).resolve().parent
RESULTS_DIR = EVAL_DIR / "results"
DATASET_PATH = EVAL_DIR / "dataset.json"
VECTORDB_DIR = PROJECT_ROOT / "data" / "vectordb"
COLLECTION_NAME = "shiori_knowledge"

CONFIG = json.loads((PROJECT_ROOT / "config.json").read_text(encoding="utf-8"))
RAG_SEARCH_URL = f"http://127.0.0.1:{CONFIG['rag']['port']}/search"

# 本番では呼び出し経路によってtop_kが異なる(identity_guard=10、
# search_knowledge=5、passive_recall=3)。評価は経路を特定せず検索の
# 生の質を見るため、search_knowledge相当の5を代表値として使う。
EVAL_TOP_K = 5


def load_query_normalization_rules() -> list[dict]:
    """Rust側のtext_transformエンジンが使う同じルールファイルを読み、
    評価時のクエリにも同じフィラー語除去を適用する(本番と同条件にするため)。
    """
    rules_path = PROJECT_ROOT / "prompts" / "transforms" / "query-normalization.json"
    data = json.loads(rules_path.read_text(encoding="utf-8"))
    return data["rules"]


def normalize_query(text: str, rules: list[dict]) -> str:
    result = text
    for rule in rules:
        result = result.replace(rule["pattern"], rule["replacement"])
    return result


def fetch_reference_texts(collection, ids: list[str]) -> list[str]:
    if not ids:
        return []
    result = collection.get(ids=ids)
    # ChromaDBのget()は要求順を保証しないため、ID→textの辞書に変換してから並べ直す
    by_id = dict(zip(result["ids"], result["documents"]))
    return [by_id[i] for i in ids if i in by_id]


def search(client: httpx.Client, query: str, top_k: int) -> list[dict]:
    resp = client.post(RAG_SEARCH_URL, json={"query": query, "top_k": top_k})
    resp.raise_for_status()
    return resp.json()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tag", required=True, help="この実行を識別するラベル(例: baseline, with_reranker)")
    parser.add_argument("--top-k", type=int, default=EVAL_TOP_K)
    args = parser.parse_args()

    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    items = dataset["items"]
    query_norm_rules = load_query_normalization_rules()

    chroma_client = chromadb.PersistentClient(path=str(VECTORDB_DIR))
    collection = chroma_client.get_collection(COLLECTION_NAME)

    precision_metric = NonLLMContextPrecisionWithReference()
    recall_metric = NonLLMContextRecall()

    per_item_results = []
    with httpx.Client(timeout=30.0) as http_client:
        for item in items:
            normalized_query = normalize_query(item["user_input"], query_norm_rules)
            retrieved = search(http_client, normalized_query, args.top_k)
            retrieved_texts = [r["text"] for r in retrieved]
            retrieved_ids = [r["id"] for r in retrieved]
            reference_texts = fetch_reference_texts(collection, item["expected_context"])

            entry = {
                "user_input": item["user_input"],
                "expected_context": item["expected_context"],
                "retrieved_ids": retrieved_ids,
            }

            if item["expected_context"]:
                if retrieved_texts:
                    sample = SingleTurnSample(
                        user_input=item["user_input"],
                        retrieved_contexts=retrieved_texts,
                        reference_contexts=reference_texts,
                    )
                    entry["context_precision"] = precision_metric.single_turn_score(sample)
                    entry["context_recall"] = recall_metric.single_turn_score(sample)
                else:
                    # リランカーの閾値フィルタ等で検索結果が0件になった場合、
                    # RAGASのNonLLMContextRecallはmax()に空リストを渡すとcrashするため、
                    # ここで「完全に見つからなかった」ケースとして明示的に0.0を入れる。
                    entry["context_precision"] = 0.0
                    entry["context_recall"] = 0.0
                entry["hit_at_k"] = bool(set(item["expected_context"]) & set(retrieved_ids))
            else:
                # 「存在しない情報」ケース: 正解チャンクがそもそも無いので
                # Precision/Recallは計算対象外。実際に検索結果が空だったか
                # (=正しく「見つからない」と判定できる状態か)だけ記録する。
                entry["context_precision"] = None
                entry["context_recall"] = None
                entry["not_found_correct"] = len(retrieved) == 0

            per_item_results.append(entry)

    scored = [e for e in per_item_results if e["context_precision"] is not None]
    not_found_items = [e for e in per_item_results if e["context_precision"] is None]

    mean_precision = sum(e["context_precision"] for e in scored) / len(scored) if scored else None
    mean_recall = sum(e["context_recall"] for e in scored) / len(scored) if scored else None
    hit_rate = sum(1 for e in scored if e["hit_at_k"]) / len(scored) if scored else None
    not_found_rate = (
        sum(1 for e in not_found_items if e["not_found_correct"]) / len(not_found_items)
        if not_found_items
        else None
    )

    summary = {
        "tag": args.tag,
        "timestamp": datetime.now().isoformat(timespec="seconds"),
        "top_k": args.top_k,
        "n_items_scored": len(scored),
        "n_items_not_found_case": len(not_found_items),
        "mean_context_precision": mean_precision,
        "mean_context_recall": mean_recall,
        "hit_rate_at_k": hit_rate,
        "not_found_correct_rate": not_found_rate,
    }

    RESULTS_DIR.mkdir(exist_ok=True)
    safe_tag = re.sub(r"[^\w\-]", "_", args.tag)
    timestamp_for_file = datetime.now().strftime("%Y%m%d_%H%M%S")
    out_path = RESULTS_DIR / f"{timestamp_for_file}_{safe_tag}.json"
    out_path.write_text(
        json.dumps({"summary": summary, "items": per_item_results}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

    history_path = RESULTS_DIR / "history.csv"
    is_new = not history_path.exists()
    with history_path.open("a", encoding="utf-8", newline="") as f:
        writer = csv.writer(f)
        if is_new:
            writer.writerow(list(summary.keys()))
        writer.writerow(list(summary.values()))

    print(json.dumps(summary, ensure_ascii=False, indent=2))
    print(f"\n詳細結果: {out_path}")
    print(f"履歴: {history_path}")


if __name__ == "__main__":
    main()
