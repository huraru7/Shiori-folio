"""Python版のチャンク分けとfrontmatterの値を、1行に1記事のJSONで書き出す(shiori-rag dump-chunks と比べる)。
使い方: rag-venvのpython py_chunks.py <services/rag> <libraryのフォルダ> <出力のJSONL>
"""
import json, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from chunking import chunk_markdown
from indexing import list_indexable_files, _is_indexable, _extract_title, _extract_summary, _extract_status, _extract_related, _extract_filter_fields, _extract_shelf_fields, _extract_recency_date
lib = Path(sys.argv[2])
out = open(sys.argv[3], "w", encoding="utf-8")
for p in sorted(list_indexable_files(lib)):
    rel = p.relative_to(lib).as_posix()
    try:
        text = p.read_text(encoding="utf-8")
    except Exception:
        continue
    chunks = [{"heading": c.heading, "text": c.text} for c in chunk_markdown(text)] if _is_indexable(text) else []
    ff = {**_extract_filter_fields(text), **_extract_shelf_fields(text)}
    fields = {"title": _extract_title(text), "summary": _extract_summary(text), "status": _extract_status(text), "related": _extract_related(text), **ff, "date": _extract_recency_date(text, ff["kind"])}
    out.write(json.dumps({"rel": rel, "fields": fields, "chunks": chunks}, ensure_ascii=False) + "\n")
