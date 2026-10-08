"""sync_indexの再開と進捗ファイルのテスト。実行: python -m unittest discover -s tests (services/rag/で)
埋め込みサーバーは使わず、get_embeddingを固定ベクトルに差し替える。
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

try:
    import chromadb

    import indexing

    HAS_DEPS = True
except ImportError:
    HAS_DEPS = False

NOTE = "---\ntitle: {n}\n---\n\n## 見出し\n\n{n}の本文\n"


class Interrupted(Exception):
    pass


@unittest.skipUnless(HAS_DEPS, "chromadbが未導入")
class SyncResumeTest(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        base = Path(self._td.name)
        self.library = base / "library"
        self.vectordb = base / "vectordb"
        (self.library / "10-projects").mkdir(parents=True)
        self.vectordb.mkdir()
        for n in "abc":
            (self.library / "10-projects" / f"{n}.md").write_text(NOTE.format(n=n), encoding="utf-8")
        self.client = chromadb.EphemeralClient()
        self.addCleanup(lambda: self.client.delete_collection("test_col") if self._has_collection() else None)

    def tearDown(self):
        self._td.cleanup()

    def _has_collection(self):
        return any(c.name == "test_col" for c in self.client.list_collections())

    def _progress(self):
        return json.loads((self.vectordb / indexing.INDEX_PROGRESS_FILENAME).read_text(encoding="utf-8"))

    def _sync(self, embed):
        with mock.patch.object(indexing, "get_embedding", side_effect=embed):
            return indexing.sync_index(self.client, mock.Mock(), self.library, self.vectordb, "test_col")

    def test_interrupted_rebuild_resumes_without_redoing_finished_files(self):
        def embed_then_fail(text, **kwargs):
            if self._progress()["done"] == 2:
                raise Interrupted()
            return [0.1, 0.2, 0.3]

        # 版が未保存(=全件の再登録)で、3件目のファイルの埋め込み中に止まる。
        with self.assertRaises(Interrupted):
            self._sync(embed_then_fail)
        self.assertEqual(indexing.load_index_schema(self.vectordb), indexing.INDEX_SCHEMA_VERSION)
        self.assertEqual(len(indexing.load_index_state(self.vectordb)), 2)
        # 止まっても進捗ファイルは残らない(待つ側が作業中と誤認しない)。
        self.assertFalse((self.vectordb / indexing.INDEX_PROGRESS_FILENAME).exists())

        # 次回は、登録済みの2件をやり直さず、残りの1件だけを登録する。
        seen = []

        def embed(text, **kwargs):
            seen.append(self._progress())
            return [0.1, 0.2, 0.3]

        _, changes = self._sync(embed)
        self.assertEqual({(p["done"], p["total"], p["phase"]) for p in seen}, {(0, 1, "syncing")})
        self.assertEqual(len(changes["added"]), 1)
        self.assertEqual(len(indexing.load_index_state(self.vectordb)), 3)

    def test_progress_file_is_written_during_sync_and_removed_after(self):
        seen = []

        def embed(text, **kwargs):
            seen.append(self._progress())
            return [0.1, 0.2, 0.3]

        self._sync(embed)
        self.assertEqual(sorted({p["done"] for p in seen}), [0, 1, 2])
        self.assertTrue(all(p["total"] == 3 and p["phase"] == "rebuilding" for p in seen))
        self.assertFalse((self.vectordb / indexing.INDEX_PROGRESS_FILENAME).exists())

    def test_unchanged_library_does_not_embed_or_write_progress(self):
        self._sync(lambda text, **kwargs: [0.1, 0.2, 0.3])
        calls = []
        self._sync(lambda text, **kwargs: calls.append(text) or [0.1, 0.2, 0.3])
        self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
