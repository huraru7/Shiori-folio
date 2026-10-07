"""素材からテキストを取り出す入口(RAGサーバーから呼ぶ)。

流れ: 拡張子の確認 → サイズ確認 → キャッシュ確認(素材のsha256) → 別プロセスで抽出 →
秘密情報の検査 → 結果(または理由つきの失敗)を返す。例外は外へ出さない。
"""
from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

from . import limits
from .formats import supported
from .secrets import find_secret

# 結果の形が変わったら上げる(古いキャッシュを使わないため)。
CACHE_VERSION = 1


@dataclass
class ExtractResult:
    ok: bool
    sections: list[tuple[str, str]] = field(default_factory=list)
    # 失敗または飛ばした理由。okでも、テキストが無いとき("テキストなし")は理由を持つ。
    reason: str = ""
    # 利用者に警告として見せるべきか(秘密情報の検出など、黙って飛ばしてはいけないもの)。
    warn: bool = False
    truncated: bool = False


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def extract_asset(path: Path, cache_dir: Path) -> ExtractResult:
    try:
        return _extract_asset(path, cache_dir)
    except Exception as e:  # 抽出の失敗でRAGサーバーを巻き込まない
        return ExtractResult(False, reason=f"抽出に失敗({type(e).__name__})")


def _extract_asset(path: Path, cache_dir: Path) -> ExtractResult:
    if not supported(path):
        return ExtractResult(False, reason="未対応の形式")
    if path.stat().st_size > limits.MAX_FILE_BYTES:
        return ExtractResult(False, reason="ファイルが大きすぎる")

    digest = _sha256(path)
    cache_file = cache_dir / f"{digest}-v{CACHE_VERSION}.json"
    if cache_file.is_file():
        try:
            data = json.loads(cache_file.read_text(encoding="utf-8"))
            return ExtractResult(
                data["ok"],
                [(s["label"], s["text"]) for s in data["sections"]],
                data.get("reason", ""),
                data.get("warn", False),
                data.get("truncated", False),
            )
        except (json.JSONDecodeError, KeyError, OSError):
            pass  # 壊れたキャッシュは作り直す

    result = _run_worker(path)
    # 一時的な失敗(タイムアウト・抽出器の未導入)はキャッシュしない。導入後に再抽出できるように。
    if result.ok or result.reason not in ("タイムアウト", "抽出器が未導入(pypdf)", "抽出器が未導入(defusedxml)"):
        try:
            cache_dir.mkdir(parents=True, exist_ok=True)
            cache_file.write_text(
                json.dumps(
                    {
                        "ok": result.ok,
                        "sections": [{"label": l, "text": t} for l, t in result.sections],
                        "reason": result.reason,
                        "warn": result.warn,
                        "truncated": result.truncated,
                    },
                    ensure_ascii=False,
                ),
                encoding="utf-8",
            )
        except OSError:
            pass
    return result


def _run_worker(path: Path) -> ExtractResult:
    try:
        proc = subprocess.run(
            [sys.executable, "-m", "extraction.worker", str(path)],
            cwd=str(Path(__file__).resolve().parents[1]),
            capture_output=True,
            timeout=limits.TIMEOUT_SECONDS,
            stdin=subprocess.DEVNULL,
        )
    except subprocess.TimeoutExpired:
        return ExtractResult(False, reason="タイムアウト")
    if len(proc.stdout) > limits.WORKER_OUTPUT_MAX_BYTES:
        return ExtractResult(False, reason="抽出結果が大きすぎる")
    try:
        data = json.loads(proc.stdout.decode("utf-8"))
    except (json.JSONDecodeError, UnicodeDecodeError):
        return ExtractResult(False, reason="抽出プロセスが異常終了")
    if not data.get("ok"):
        return ExtractResult(False, reason=data.get("reason", "抽出に失敗"))

    sections = [(s["label"], s["text"]) for s in data["sections"]]
    if not sections:
        return ExtractResult(True, [], reason="テキストなし")
    secret = find_secret("\n".join(t for _, t in sections))
    if secret:
        # 秘密情報らしきものが含まれる素材は、索引に入れない(中身の断片も記録に残さない)。
        return ExtractResult(False, reason=f"秘密情報らしき文字列を検出({secret})", warn=True)
    return ExtractResult(True, sections, truncated=bool(data.get("truncated")))
