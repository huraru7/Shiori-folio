"""知識の棚(knowledge/)の索引のテスト。実行: python -m unittest discover -s tests (services/rag/で)
埋め込みサーバーは使わず、get_embeddingを固定ベクトルに差し替える。
"""
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

try:
    import chromadb

    import indexing
    from library_path import resolve_shelf_dir

    HAS_DEPS = True
except ImportError:
    HAS_DEPS = False

SHELF_NOTE = """---
title: 無限小は0と何が違うか
type: resource
tags:
- 数学
summary: 無限小は0ではなく、どの正の実数よりも小さい別の種類の数。
index: true
status: new
source: AIとの対話
verified: unverified
learned: '2026-10-17'
topic: 数学
date: '2026-10-17T16:00:00+09:00'
---

## 要点

無限小は0ではない。
"""

LIBRARY_NOTE = "---\ntitle: 通常の記事\nsource: 本の名前\n---\n\n## 見出し\n\n本文\n"


@unittest.skipUnless(HAS_DEPS, "chromadbが未導入")
class ShelfIndexTest(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.base = Path(self._td.name)
        self.shelf = self.base / "knowledge"
        self.state = self.base / "vectordb" / "shelf"
        self.shelf.mkdir()
        self.state.mkdir(parents=True)
        self.client = chromadb.EphemeralClient()
        self.addCleanup(self._drop)
        patcher = mock.patch.object(indexing, "get_embedding", return_value=[0.1, 0.2, 0.3])
        patcher.start()
        self.addCleanup(patcher.stop)

    def _drop(self):
        for name in ("test_shelf", "test_lib"):
            if any(c.name == name for c in self.client.list_collections()):
                self.client.delete_collection(name)
        self._td.cleanup()

    def test_resolve_shelf_dir_prefers_portable_layout(self):
        (self.base / "knowledge").mkdir(exist_ok=True)
        self.assertEqual(resolve_shelf_dir(self.base), self.base / "knowledge")

    def test_resolve_shelf_dir_falls_back_to_sibling(self):
        root = self.base / "system"
        root.mkdir()
        self.assertEqual(resolve_shelf_dir(root), self.base / "knowledge")

    def test_shelf_fields_go_to_metadata(self):
        (self.shelf / "math-infinitesimal.md").write_text(SHELF_NOTE, encoding="utf-8")
        collection, result = indexing.sync_index(self.client, mock.Mock(), self.shelf, self.state, "test_shelf")
        self.assertEqual(result["added"], ["math-infinitesimal.md"])
        meta = collection.get(include=["metadatas"])["metadatas"][0]
        self.assertEqual(meta["verified"], "unverified")
        self.assertEqual(meta["topic"], "数学")
        self.assertEqual(meta["learned"], "2026-10-17")
        self.assertEqual(meta["cited_from"], "AIとの対話")
        # frontmatterのsourceはファイル名のsourceを上書きしない。
        self.assertEqual(meta["source"], "math-infinitesimal.md")

    def test_library_articles_get_no_shelf_fields(self):
        lib = self.base / "library"
        (lib / "30-resources").mkdir(parents=True)
        (lib / "30-resources" / "a.md").write_text(LIBRARY_NOTE, encoding="utf-8")
        vec = self.base / "vectordb"
        collection, _ = indexing.sync_index(self.client, mock.Mock(), lib, vec, "test_lib")
        meta = collection.get(include=["metadatas"])["metadatas"][0]
        for key in ("verified", "topic", "learned", "cited_from"):
            self.assertNotIn(key, meta)
        self.assertEqual(meta["source"], "a.md")

    def test_shelf_and_library_use_separate_state(self):
        (self.shelf / "math-infinitesimal.md").write_text(SHELF_NOTE, encoding="utf-8")
        indexing.sync_index(self.client, mock.Mock(), self.shelf, self.state, "test_shelf")
        self.assertTrue((self.state / indexing.INDEX_STATE_FILENAME).is_file())
        self.assertFalse((self.base / "vectordb" / indexing.INDEX_STATE_FILENAME).exists())


if __name__ == "__main__":
    unittest.main()
