"""抽出の実行プロセス。runner.pyが別プロセスとして起動し、結果をJSONで標準出力へ返す。

RAGサーバーとは別プロセスにしておくことで、悪意のある・壊れた素材で抽出がハング・暴走・
クラッシュしても、サーバーは影響を受けない(タイムアウトで親が殺す)。
使い方: python -m extraction.worker <素材のパス>
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

from . import limits
from .formats import REGISTRY, ExtractionError


def _apply_limits() -> None:
    """POSIXではCPU時間とメモリの上限をかける(Windowsではタイムアウトのみ)。"""
    try:
        import resource

        resource.setrlimit(resource.RLIMIT_CPU, (limits.TIMEOUT_SECONDS, limits.TIMEOUT_SECONDS + 5))
        if sys.platform.startswith("linux"):
            cap = 2 * 1024 * 1024 * 1024
            resource.setrlimit(resource.RLIMIT_AS, (cap, cap))
    except (ImportError, ValueError, OSError):
        pass


def main() -> int:
    _apply_limits()
    path = Path(sys.argv[1])
    try:
        ext = path.suffix.lower().lstrip(".")
        extractor = REGISTRY.get(ext)
        if extractor is None:
            raise ExtractionError("未対応の形式", ext)
        if path.stat().st_size > limits.MAX_FILE_BYTES:
            raise ExtractionError("ファイルが大きすぎる")
        sections = extractor(path)
        total = 0
        out = []
        truncated = False
        for label, text in sections:
            text = text.strip()
            if not text:
                continue
            if total + len(text) > limits.MAX_CHARS:
                text = text[: limits.MAX_CHARS - total]
                truncated = True
            out.append({"label": label, "text": text})
            total += len(text)
            if truncated:
                break
        print(json.dumps({"ok": True, "sections": out, "truncated": truncated}, ensure_ascii=False))
    except ExtractionError as e:
        print(json.dumps({"ok": False, "reason": e.reason, "detail": str(e)[:200]}, ensure_ascii=False))
    except Exception as e:  # 想定外の失敗も、親には理由つきの失敗として返す
        print(json.dumps({"ok": False, "reason": "抽出に失敗", "detail": f"{type(e).__name__}: {str(e)[:150]}"}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
