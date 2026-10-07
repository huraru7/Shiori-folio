"""passage.py・lexical.pyの単体テスト。実行: python -m unittest discover -s tests (services/rag/で)"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from lexical import LexicalDoc, LexicalIndex, bigrams, matches_where  # noqa: E402
from passage import passage_text  # noqa: E402


class PassageTextTest(unittest.TestCase):
    def test_タイトルと要約と見出しを前置する(self):
        self.assertEqual(
            passage_text("題名", "要約です", "背景", "本文"),
            "題名。要約です\n背景: 本文",
        )

    def test_見出しなしは本文だけ(self):
        self.assertEqual(passage_text("題名", "", "(見出しなし)", "本文"), "題名\n本文")

    def test_タイトルも要約も無ければ従来の形(self):
        self.assertEqual(passage_text("", "", "背景", "本文"), "背景: 本文")
        self.assertEqual(passage_text("", "", "(見出しなし)", "本文"), "本文")


class LexicalTest(unittest.TestCase):
    def _index(self, texts, metas=None):
        docs = [LexicalDoc(id=f"d{i}", document=t, metadata=(metas[i] if metas else {})) for i, t in enumerate(texts)]
        return LexicalIndex(docs, texts)

    def test_bigram(self):
        self.assertEqual(bigrams("起動 が"), ["起動", "動が"])
        self.assertEqual(bigrams("あ"), ["あ"])
        self.assertEqual(bigrams(""), [])

    def test_語彙が重なる文書が上位に来る(self):
        idx = self._index(["バックエンド起動の並列化について", "ランクと称号の分割", "天気の話"])
        top = idx.top("起動の並列化", 3)
        self.assertEqual(top[0].id, "d0")
        self.assertNotIn("d2", [d.id for d in top])

    def test_重なりがなければ空(self):
        idx = self._index(["あいう", "えおか"])
        self.assertEqual(idx.top("zzz", 5), [])

    def test_絞り込みに合わない文書は除く(self):
        idx = self._index(
            ["起動の話", "起動の話"],
            [{"status": "deprecated"}, {"status": "new"}],
        )
        ok = lambda meta: matches_where(meta, {"status": {"$ne": "deprecated"}})  # noqa: E731
        self.assertEqual([d.id for d in idx.top("起動", 5, accept=ok)], ["d1"])

    def test_件数の上限(self):
        idx = self._index([f"起動{i}" for i in range(10)])
        self.assertEqual(len(idx.top("起動", 3)), 3)

    def test_where句(self):
        meta = {"project": "shiori", "status": "new"}
        self.assertTrue(matches_where(meta, None))
        self.assertTrue(matches_where(meta, {"project": "shiori"}))
        self.assertFalse(matches_where(meta, {"project": "other"}))
        self.assertTrue(matches_where(meta, {"$and": [{"project": "shiori"}, {"status": {"$ne": "deprecated"}}]}))
        self.assertFalse(matches_where(meta, {"$and": [{"project": "shiori"}, {"status": {"$ne": "new"}}]}))
        # statusが無い文書(メモ等)は、$neの絞り込みで落とさない
        self.assertTrue(matches_where({}, {"status": {"$ne": "deprecated"}}))


if __name__ == "__main__":
    unittest.main()
