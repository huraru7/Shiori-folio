"""共通記憶MD(kind: memory、詩織Ver4.1)の検索順位の補正。

app.pyは読み込むだけで実機のベクトルDBを開くため、単体テストできるよう純粋な関数をここに分ける。
"""
from __future__ import annotations


def promote_project_memory(order: list[str], files: dict[str, dict], query: str) -> list[str]:
    """クエリにプロジェクト名(project id)が含まれるとき、そのプロジェクトのmemoryを
    先頭へ移す。memoryは作業再開時に最初に読むものなので、プロジェクト名の検索では
    他の記事より先に返す。それ以外の並びは変えない(複数該当するときも元の順)。

    orderはファイル(source)の並び、filesはsourceごとに"kind"と"project"を持つ辞書。
    """
    q = query.casefold()
    promoted = [
        s for s in order
        if files[s].get("kind") == "memory"
        and files[s].get("project")
        and files[s]["project"].casefold() in q
    ]
    return promoted + [s for s in order if s not in promoted]
