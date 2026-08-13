"""embedding-server(llama-server --embedding)への問い合わせ。ポート番号は
config.jsonのembedding.portから読む(Rust側と設定を二重管理しないため)。
"""
from __future__ import annotations

import json
from pathlib import Path

import httpx

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_CONFIG = json.loads((_PROJECT_ROOT / "config.json").read_text(encoding="utf-8"))
EMBEDDING_SERVER_URL = f"http://127.0.0.1:{_CONFIG['embedding']['port']}/v1/embeddings"


def get_embedding(
    text: str, client: httpx.Client | None = None, *, is_query: bool = False
) -> list[float]:
    # nomic-embed-textはクエリ/文書それぞれに専用のプレフィックスを付けると
    # 検索精度が上がる仕様があるため、用途に応じて付与する
    prefix = "search_query: " if is_query else "search_document: "
    owns_client = client is None
    client = client or httpx.Client(timeout=30.0)
    try:
        resp = client.post(EMBEDDING_SERVER_URL, json={"input": prefix + text})
        resp.raise_for_status()
        data = resp.json()
        return data["data"][0]["embedding"]
    finally:
        if owns_client:
            client.close()
