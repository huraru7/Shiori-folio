"""Markdownの見出し単位でのチャンク分割。

仕様: 見出し単位を優先し、1チャンクあたり目安400〜500トークン、
見出し1つの内容がそれを超える場合のみオーバーラップ50トークンで分割する。
"""
from __future__ import annotations

import re
from dataclasses import dataclass

import tiktoken

_ENCODING = tiktoken.get_encoding("cl100k_base")

CHUNK_TARGET_TOKENS = 500
OVERLAP_TOKENS = 50

_HEADING_RE = re.compile(r"^(#{1,6})\s+(.*)$")


@dataclass
class Chunk:
    heading: str
    text: str


def _count_tokens(text: str) -> int:
    return len(_ENCODING.encode(text))


def _split_by_heading(markdown: str) -> list[Chunk]:
    lines = markdown.splitlines()
    sections: list[Chunk] = []
    current_heading = "(見出しなし)"
    current_lines: list[str] = []

    def flush():
        content = "\n".join(current_lines).strip()
        if content:
            sections.append(Chunk(heading=current_heading, text=content))

    for line in lines:
        match = _HEADING_RE.match(line)
        if match:
            flush()
            current_heading = match.group(2).strip()
            current_lines = []
        else:
            current_lines.append(line)
    flush()
    return sections


def _split_long_section(section: Chunk) -> list[Chunk]:
    tokens = _ENCODING.encode(section.text)
    if len(tokens) <= CHUNK_TARGET_TOKENS:
        return [section]

    parts: list[Chunk] = []
    start = 0
    step = CHUNK_TARGET_TOKENS - OVERLAP_TOKENS
    while start < len(tokens):
        window = tokens[start : start + CHUNK_TARGET_TOKENS]
        parts.append(Chunk(heading=section.heading, text=_ENCODING.decode(window)))
        if start + CHUNK_TARGET_TOKENS >= len(tokens):
            break
        start += step
    return parts


def chunk_markdown(markdown: str) -> list[Chunk]:
    """見出し単位で分割し、長すぎるセクションはオーバーラップ付きでさらに分割する。"""
    sections = _split_by_heading(markdown)
    chunks: list[Chunk] = []
    for section in sections:
        chunks.extend(_split_long_section(section))
    return chunks
