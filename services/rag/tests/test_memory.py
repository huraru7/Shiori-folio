"""共通記憶MD(kind: memory、詩織Ver4.1)の索引と順位のテスト。
実行: python -m unittest discover -s tests (services/rag/で)
埋め込みサーバーは使わず、get_embeddingを固定ベクトルに差し替える。
"""
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from memory_rank import promote_project_memory  # noqa: E402

try:
    import chromadb

    import indexing

    HAS_DEPS = True
except ImportError:
    HAS_DEPS = False

MEMORY = (
    "---\ntitle: {p}のmemory\ntype: area\nkind: memory\nproject: {p}\n"
    "date: 2026-01-01T00:00:00+09:00\nupdated: {updated}\n---\n\n## 現在の状態\n\n{p}の{body}\n"
)


class SourceNameTest(unittest.TestCase):
    @unittest.skipUnless(HAS_DEPS, "chromadbが未導入")
    def test_memoryだけprojectを含む名前になる(self):
        self.assertEqual(indexing.source_name_for("20-areas/shiori/memory.md"), "shiori/memory.md")
        self.assertEqual(indexing.source_name_for("10-projects\\tanker\\memory.md"), "tanker/memory.md")
        # 全体用・通常記事・深い階層のmemory.md(アーカイブ等)は、ファイル名のまま。
        self.assertEqual(indexing.source_name_for("40-profile/memory-global.md"), "memory-global.md")
        self.assertEqual(indexing.source_name_for("20-areas/shiori/journal/a.md"), "a.md")
        self.assertEqual(indexing.source_name_for("90-archive/20-areas/shiori/memory.md"), "memory.md")

    @unittest.skipUnless(HAS_DEPS, "chromadbが未導入")
    def test_memoryの新しさはupdatedで見る(self):
        text = MEMORY.format(p="shiori", updated="2026-10-10T10:00:00+09:00", body="x")
        self.assertEqual(indexing._extract_recency_date(text, "memory"), "2026-10-10T10:00:00+09:00")
        # memory以外はdateのまま(updatedを持っていても見ない)。
        self.assertEqual(indexing._extract_recency_date(text, "journal"), "2026-01-01T00:00:00+09:00")


class PromoteTest(unittest.TestCase):
    FILES = {
        "a.md": {"kind": "journal", "project": "shiori"},
        "shiori/memory.md": {"kind": "memory", "project": "shiori"},
        "tanker/memory.md": {"kind": "memory", "project": "tanker"},
        "memory-global.md": {"kind": "memory", "project": ""},
    }
    ORDER = ["a.md", "tanker/memory.md", "shiori/memory.md", "memory-global.md"]

    def test_クエリのプロジェクトのmemoryを先頭へ(self):
        self.assertEqual(
            promote_project_memory(self.ORDER, self.FILES, "Shiori の作業状況"),
            ["shiori/memory.md", "a.md", "tanker/memory.md", "memory-global.md"],
        )

    def test_プロジェクト名が無いクエリでは並びを変えない(self):
        self.assertEqual(promote_project_memory(self.ORDER, self.FILES, "検索の仕組み"), self.ORDER)

    def test_projectが空のmemoryは引き上げない(self):
        self.assertEqual(promote_project_memory(self.ORDER, self.FILES, ""), self.ORDER)


@unittest.skipUnless(HAS_DEPS, "chromadbが未導入")
class MemoryIndexTest(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        base = Path(self._td.name)
        self.library = base / "library"
        self.vectordb = base / "vectordb"
        self.vectordb.mkdir()
        for p in ("shiori", "tanker"):
            d = self.library / "20-areas" / p
            d.mkdir(parents=True)
            (d / "memory.md").write_text(
                MEMORY.format(p=p, updated="2026-10-01T00:00:00+09:00", body="初版"), encoding="utf-8"
            )
        self.client = chromadb.EphemeralClient()
        self.addCleanup(lambda: self.client.delete_collection("test_mem"))
        self.addCleanup(self._td.cleanup)

    def _sync(self):
        with mock.patch.object(indexing, "get_embedding", return_value=[0.1, 0.2, 0.3]):
            return indexing.sync_index(self.client, mock.Mock(), self.library, self.vectordb, "test_mem")

    def _reindex(self, rel):
        with mock.patch.object(indexing, "get_embedding", return_value=[0.1, 0.2, 0.3]):
            return indexing.reindex_single_file(
                self.client, mock.Mock(), self.library, self.vectordb, "test_mem", rel
            )

    def _sources(self):
        col = self.client.get_collection("test_mem")
        return sorted({m["source"] for m in col.get(include=["metadatas"])["metadatas"]})

    def test_同名のmemoryがプロジェクトごとに別のsourceで共存する(self):
        self._sync()
        self.assertEqual(self._sources(), ["shiori/memory.md", "tanker/memory.md"])
        col = self.client.get_collection("test_mem")
        ids = col.get()["ids"]
        self.assertEqual(len(ids), len(set(ids)))

    def test_片方を再登録しても他方のチャンクは消えない(self):
        self._sync()
        (self.library / "20-areas" / "shiori" / "memory.md").write_text(
            MEMORY.format(p="shiori", updated="2026-10-02T00:00:00+09:00", body="更新後"), encoding="utf-8"
        )
        self._reindex("20-areas/shiori/memory.md")
        self.assertEqual(self._sources(), ["shiori/memory.md", "tanker/memory.md"])
        col = self.client.get_collection("test_mem")
        docs = col.get(where={"source": "shiori/memory.md"})["documents"]
        self.assertTrue(any("更新後" in d for d in docs) and not any("初版" in d for d in docs))
        metas = col.get(where={"source": "shiori/memory.md"})["metadatas"]
        self.assertEqual({m["date"] for m in metas}, {"2026-10-02T00:00:00+09:00"})

    def test_旧形式のsourceのチャンクは再登録で置き換わる(self):
        col = self.client.get_or_create_collection("test_mem")
        col.upsert(
            ids=["20-areas-memory-0"], documents=["旧"], embeddings=[[0.1, 0.2, 0.3]],
            metadatas=[{"source": "memory.md", "relative_path": "20-areas/shiori/memory.md"}],
        )
        self._reindex("20-areas/shiori/memory.md")
        self.assertEqual(self._sources(), ["shiori/memory.md"])

    def test_片方のファイルを消すと同期でその分だけ消える(self):
        self._sync()
        (self.library / "20-areas" / "tanker" / "memory.md").unlink()
        self._sync()
        self.assertEqual(self._sources(), ["shiori/memory.md"])


if __name__ == "__main__":
    unittest.main()
