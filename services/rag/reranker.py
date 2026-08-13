"""日本語Cross-Encoderによるリランキング。

ベクトル検索(embedding距離)だけでは、この規模のデータで正しいチャンクと
無関係なチャンクの距離が逆転することがあると実測で分かっている
(docs/voice-consistency-policy.mdの4a節参照)。Cross-Encoderはクエリと
チャンクのペアを直接読んで関連度を判定するため、embeddingの距離だけに
頼るより精度が高いことが期待される。

モデルはCPU実行(VRAM消費ゼロ)。初回呼び出し時に遅延ロードし、以後は
プロセス内で保持する(uvicornの1プロセスにつき1回だけロードすれば良い)。
"""
from __future__ import annotations

from sentence_transformers import CrossEncoder

MODEL_NAME = "hotchpotch/japanese-reranker-xsmall-v2"

_model: CrossEncoder | None = None


def _get_model() -> CrossEncoder:
    global _model
    if _model is None:
        _model = CrossEncoder(MODEL_NAME, device="cpu")
    return _model


def rerank(query: str, passages: list[str]) -> list[float]:
    """queryと各passageの関連度スコアを、passagesと同じ順序のリストで返す。
    スコアはCrossEncoderの生の出力(だいたい0〜1の範囲だが確率として正規化
    されている保証はない。閾値は実際のデータで確認して決める)。
    """
    if not passages:
        return []
    model = _get_model()
    pairs = [(query, passage) for passage in passages]
    scores = model.predict(pairs)
    return [float(s) for s in scores]
