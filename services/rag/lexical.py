"""語彙の一致による検索(文字bigramのBM25)。埋め込み検索が言い回しの違いで取りこぼした
チャンクを、候補に加えるために使う(詩織Ver3.8)。

形態素解析器(MeCab等)は使わない。日本語は文字bigramで分かち書きなしにBM25が成立し、
追加依存がなく、Mac/Windowsで同じに動くため。最終的な順位はリランカーが決めるので、ここで
欲しいのは「正解を候補に入れる」ことだけで、順位の精度は要らない。
"""
from __future__ import annotations

import math
import re
import unicodedata
from collections import Counter
from dataclasses import dataclass

K1 = 1.5
B = 0.75


def bigrams(text: str) -> list[str]:
    s = re.sub(r"\s+", "", unicodedata.normalize("NFC", text).lower())
    return [s[i : i + 2] for i in range(len(s) - 1)] or ([s] if s else [])


@dataclass
class LexicalDoc:
    id: str
    document: str
    metadata: dict


class LexicalIndex:
    def __init__(self, docs: list[LexicalDoc], texts: list[str]):
        self.docs = docs
        self._tf = [Counter(bigrams(t)) for t in texts]
        self._len = [sum(tf.values()) for tf in self._tf]
        self._avg = (sum(self._len) / len(self._len)) if self._len else 0.0
        df: Counter[str] = Counter()
        for tf in self._tf:
            df.update(tf.keys())
        n = len(docs)
        self._idf = {w: math.log(1 + (n - c + 0.5) / (c + 0.5)) for w, c in df.items()}

    def top(self, query: str, n: int, accept=lambda meta: True) -> list[LexicalDoc]:
        """queryに語彙が重なるチャンクを、BM25の高い順にn件返す(スコア0は除く)。
        acceptは、メタデータの絞り込み(status・project等)の判定関数。
        """
        qs = bigrams(query)
        scored: list[tuple[float, int]] = []
        for i, (tf, ln) in enumerate(zip(self._tf, self._len)):
            if not accept(self.docs[i].metadata):
                continue
            s = 0.0
            for w in qs:
                f = tf.get(w, 0)
                if f:
                    s += self._idf.get(w, 0.0) * f * (K1 + 1) / (f + K1 * (1 - B + B * ln / self._avg))
            if s > 0:
                scored.append((s, i))
        scored.sort(reverse=True)
        return [self.docs[i] for _, i in scored[:n]]


def matches_where(meta: dict, where: dict | None) -> bool:
    """ChromaDBのwhere句のうち、この検索が使う形(完全一致・$ne・$in・$nin・$and)だけを判定する。
    未対応の演算子が来たときは、候補から落とさず通す(語彙検索は候補を足すだけで、
    絞り込みの最終判断は密ベクトル側のChromaDBが担うため、取りこぼしより混入を選ぶ)。
    """
    if not where:
        return True
    for key, cond in where.items():
        if key == "$and":
            if not all(matches_where(meta, sub) for sub in cond):
                return False
        elif isinstance(cond, dict):
            for op, val in cond.items():
                if op == "$ne" and meta.get(key) == val:
                    return False
                if op == "$in" and meta.get(key) not in val:
                    return False
                if op == "$nin" and meta.get(key) in val:
                    return False
        elif meta.get(key) != cond:
            return False
    return True
