"""Ver1.0由来のデータ(library/profile/, library/memo/)を、Ver2.0のfrontmatter
形式(title/type/tags/project/author)に書き換えた上でshiori-save CLIへ渡し、
Ver2.0のディレクトリ体系(50-reference/, 30-knowledge/等)へ移動する、一回限りの
移行スクリプト(詩織Ver2.0設計指示書v3、Phase 9)。

配置先の決定・タグ/プロジェクトの自己登録は一切ここでは行わず、Phase 5で実装
済みのshiori-save CLI(src-tauri/src/bin/shiori_save.rs)にそのまま委譲する
(decide_destination等のロジックをここで二重実装しない)。このスクリプトの
役割は、Ver1.0のfrontmatter(type: プロフィール/メモ等、日本語でVer2.0のtype
enumに存在しない値)をVer2.0のtype enumへ書き換えるところまで。

事前に `cargo build --bin shiori_save`(src-tauri/配下)でCLIをビルドしておくこと。
PyYAMLが必要(services/rag/.venvに導入済みのため、そちらのpythonで実行するのが
簡単: `services/rag/.venv/bin/python scripts/migrate_v1_data.py`)。

--library-root/--shiori-saveを指定すると、開発ツリー以外(ポータブル版・USB上の
別インストール等)のlibrary/にも同じ移行処理を適用できる(2026-08-15、USB版
Ver2.0移行で追加)。省略時は開発ツリー(system/の兄弟のlibrary/、および
src-tauri/target/debug/shiori_save)を対象にする。

実行(system/直下から): python scripts/migrate_v1_data.py [--dry-run]
                        [--library-root PATH] [--shiori-save PATH]
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

import yaml

SYSTEM_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LIBRARY_ROOT = SYSTEM_ROOT.parent / "library"

# 移行元ディレクトリ(library/直下の旧棚)ごとの、Ver2.0でのtype/projectの
# 割り当てルール。garden/portfolioは調査時点でファイル0件だったが、将来
# データが入る場合に備えprofileと同じreference扱いで用意しておく。
MIGRATIONS = [
    {"dir": "profile", "type": "reference", "project": "profile", "extra_tags": []},
    {"dir": "garden", "type": "reference", "project": "garden", "extra_tags": []},
    {"dir": "portfolio", "type": "reference", "project": "portfolio", "extra_tags": []},
    # memoは日々の一言メモ(日記的な短文)のため、experienceとして通常の
    # ルーティングに乗せる(project未指定→30-knowledge/general行き)。
    {"dir": "memo", "type": "experience", "project": None, "extra_tags": ["日記"]},
]

FRONTMATTER_RE = re.compile(r"\A﻿?---\r?\n(.*?\r?\n)---\r?\n?(.*)\Z", re.DOTALL)


def parse_frontmatter(text: str) -> tuple[dict, str] | None:
    m = FRONTMATTER_RE.match(text)
    if not m:
        return None
    fm_block, body = m.group(1), m.group(2)
    fields = yaml.safe_load(fm_block) or {}
    if not isinstance(fields, dict):
        return None
    return fields, body


V2_KEYS = {"title", "type", "tags", "project", "author"}


def build_v2_frontmatter(fields: dict, migration: dict, fallback_title: str) -> dict:
    # "note"はタイトルの代用にはしない(要レビュー等の長い注記であることが
    # 多く、タイトルとしては不適切なため)。titleが無ければファイル名を使う。
    title = fields.get("title") or fallback_title
    raw_tags = fields.get("tags") or []
    if isinstance(raw_tags, str):
        raw_tags = [raw_tags]
    tags = [str(t) for t in raw_tags] + migration["extra_tags"]
    if not tags:
        tags = ["未分類"]

    fm: dict = {
        "title": str(title),
        "type": migration["type"],
        "tags": tags,
    }
    if migration["project"]:
        fm["project"] = migration["project"]
    fm["author"] = "claude-code"

    # note/date等、Ver2.0のtitle/type/tags/project/author以外のVer1.0
    # frontmatterフィールドは情報として失わず、そのままextraとして引き継ぐ
    # (shiori_save.rs側もFrontmatter.extraで未知フィールドを保持する設計と
    # 揃えている)。
    for key, value in fields.items():
        if key not in V2_KEYS and key not in fm:
            fm[key] = value
    return fm


def migrate_file(
    path: Path, migration: dict, shiori_save: Path, dry_run: bool, library_root: Path
) -> bool:
    text = path.read_text(encoding="utf-8")
    parsed = parse_frontmatter(text)
    if parsed is None:
        print(f"[スキップ] frontmatterが見つかりません: {path.relative_to(library_root)}")
        return False
    fields, body = parsed

    new_fm = build_v2_frontmatter(fields, migration, fallback_title=path.stem)
    new_frontmatter_yaml = yaml.safe_dump(new_fm, allow_unicode=True, sort_keys=False)
    new_content = f"---\n{new_frontmatter_yaml}---\n{body.lstrip(chr(10))}"

    project_note = f", project={migration['project']}" if migration["project"] else ""
    print(f"[移行] {path.relative_to(library_root)} -> type={migration['type']}{project_note}")
    if dry_run:
        return True

    path.write_text(new_content, encoding="utf-8")
    result = subprocess.run(
        [str(shiori_save), str(path)], capture_output=True, text=True
    )
    if result.returncode != 0:
        print(f"  shiori-save失敗: {result.stderr.strip()}")
        return False
    print(f"  {result.stdout.strip()}")
    return True


def resolve_shiori_save_binary() -> Path:
    exe_name = "shiori_save.exe" if sys.platform == "win32" else "shiori_save"
    return SYSTEM_ROOT / "src-tauri" / "target" / "debug" / exe_name


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="書き換え・移動を行わず対象を表示するだけ")
    parser.add_argument(
        "--library-root", type=Path, default=None, help="移行対象のlibrary/(省略時は開発ツリー)"
    )
    parser.add_argument(
        "--shiori-save", type=Path, default=None, help="shiori-save実行ファイル(省略時は開発ツリーのdebugビルド)"
    )
    args = parser.parse_args()

    library_root = args.library_root or DEFAULT_LIBRARY_ROOT
    shiori_save = args.shiori_save or resolve_shiori_save_binary()
    if not args.dry_run and not shiori_save.is_file():
        raise SystemExit(
            f"shiori-saveバイナリが見つかりません: {shiori_save}\n"
            "先にsrc-tauri/で `cargo build --bin shiori_save` を実行するか、"
            "--shiori-saveで既存バイナリのパスを指定してください。"
        )

    migrated = 0
    skipped = 0
    for migration in MIGRATIONS:
        src_dir = library_root / migration["dir"]
        if not src_dir.is_dir():
            continue
        for path in sorted(src_dir.glob("*.md")):
            ok = migrate_file(path, migration, shiori_save, args.dry_run, library_root)
            if ok:
                migrated += 1
            else:
                skipped += 1

    mode = "(dry-run、実際の変更なし)" if args.dry_run else ""
    print(f"完了{mode}: 移行対象{migrated}件 / スキップ{skipped}件")


if __name__ == "__main__":
    main()
