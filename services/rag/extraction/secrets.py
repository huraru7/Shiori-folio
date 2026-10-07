"""抽出したテキストに秘密情報らしき文字列が含まれていないかの検査。

見つかったら、その素材は索引に入れない(誤検知で検索に出ない素材が出る代わりに、漏れを防ぐ)。
ファイル名の検査(shiori-saveの秘密情報チェック)と合わせて二重にする。
"""
from __future__ import annotations

import re

# (名前, パターン)。名前は警告の理由に使う(一致した文字列そのものは記録しない)。
PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("秘密鍵のヘッダー", re.compile(r"-----BEGIN (?:[A-Z]+ )?PRIVATE KEY(?: BLOCK)?-----")),
    ("AWSアクセスキー", re.compile(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b")),
    ("OpenAI/Anthropic系のAPIキー", re.compile(r"\bsk-[A-Za-z0-9_\-]{20,}")),
    ("GitHubトークン", re.compile(r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{30,}|\bgithub_pat_[A-Za-z0-9_]{30,}")),
    ("Slackトークン", re.compile(r"\bxox[abprs]-[A-Za-z0-9\-]{10,}")),
    ("Googleアクセスキー", re.compile(r"\bAIza[0-9A-Za-z_\-]{35}\b")),
    (
        "鍵・パスワードの代入",
        re.compile(
            r"(?i)\b(?:api[_-]?key|secret[_-]?key|client[_-]?secret|access[_-]?token|auth[_-]?token|passwd|password)"
            r"\s*[:=]\s*[\"']?[A-Za-z0-9/+_\-]{16,}"
        ),
    ),
]


def find_secret(text: str) -> str | None:
    """最初に見つかった秘密情報の種類(名前)を返す。無ければNone。"""
    for name, pattern in PATTERNS:
        if pattern.search(text):
            return name
    return None
