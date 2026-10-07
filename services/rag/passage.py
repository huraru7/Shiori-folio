"""検索用の本文(パッセージ)の組み立て。埋め込み(indexing.py)・リランク・語彙検索(app.py)の
3か所で同じ形にそろえるため、ここに1つだけ置く(詩織Ver3.8)。

チャンクは見出し単位で切るため、「背景」「対応方針」のようなチャンク単体には、その記事が
何についてのものかが入らない。記事のタイトルと要約(frontmatter)を前置して、言い回しの違う
質問(「起動が遅かった件」と記事の「バックエンド起動の並列化」など)でも当たるようにする。
実測(2026-10-07、評価セット39問)で、リランク入力だけの前置でhit@5が0.846→0.923、
埋め込みへの前置とBM25の併用まで含めると0.949になった。
"""
from __future__ import annotations

NO_HEADING = "(見出しなし)"


def passage_text(title: str, summary: str, heading: str, text: str) -> str:
    head = "。".join(x for x in (title, summary) if x)
    body = text if heading in ("", NO_HEADING) else f"{heading}: {text}"
    return f"{head}\n{body}" if head else body
