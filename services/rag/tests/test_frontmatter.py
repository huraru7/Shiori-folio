"""indexing.pyのfrontmatter抽出の単体テスト。実行: python -m unittest discover -s tests (services/rag/で)"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from indexing import _extract_filter_fields  # noqa: E402
from lexical import matches_where  # noqa: E402

ARTICLE = """---
title: 'SSD取り外し: 手順'
type: area
kind: resource
tags:
- 詩織
project: shiori
status: new
---

## 本文
type: これは本文なので拾わない
"""


class FilterFieldsTest(unittest.TestCase):
    def test_projectとtypeを取り出す(self):
        self.assertEqual(
            _extract_filter_fields(ARTICLE),
            {"project": "shiori", "type": "area", "author": "", "kind": "resource"},
        )

    def test_frontmatterが無ければ空文字列(self):
        self.assertEqual(
            _extract_filter_fields("## 見出し\nproject: shiori\n"),
            {"project": "", "type": "", "author": "", "kind": ""},
        )

    def test_取り出した値で絞り込みが一致する(self):
        # /search_libraryのfilterが作るwhere句と同じ形
        meta = {"source": "a.md", **_extract_filter_fields(ARTICLE)}
        self.assertTrue(matches_where(meta, {"project": "shiori"}))
        self.assertTrue(matches_where(meta, {"$and": [{"project": "shiori"}, {"type": "area"}]}))
        self.assertFalse(matches_where(meta, {"project": "tanker"}))

    def test_図書館の絞り込みの演算子(self):
        meta = {"source_category": "20-areas", "status": "new", "kind": "resource"}
        self.assertTrue(matches_where(meta, {"source_category": {"$in": ["10-projects", "20-areas"]}}))
        self.assertFalse(matches_where(meta, {"source_category": {"$in": ["90-archive"]}}))
        self.assertTrue(matches_where(meta, {"status": {"$nin": ["outdated", "deprecated"]}}))
        stale = {**meta, "status": "outdated"}
        self.assertFalse(matches_where(stale, {"status": {"$nin": ["outdated", "deprecated"]}}))


if __name__ == "__main__":
    unittest.main()
