"""extraction/の単体テスト。実行: python -m unittest discover -s tests (services/rag/で)
抽出器の依存(pypdf・defusedxml)が未導入の環境では、それを使うテストを飛ばす。
"""
import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from extraction import limits  # noqa: E402
from extraction.formats import ExtractionError, extract_docx, extract_html, extract_text_file, extract_xlsx, extract_pptx  # noqa: E402
from extraction.runner import extract_asset  # noqa: E402
from extraction.secrets import find_secret  # noqa: E402

try:
    import defusedxml  # noqa: F401

    HAS_DEFUSEDXML = True
except ImportError:
    HAS_DEFUSEDXML = False
try:
    import pypdf  # noqa: F401

    HAS_PYPDF = True
except ImportError:
    HAS_PYPDF = False


class TmpCase(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.dir = Path(self._td.name)
        self.cache = self.dir / "cache"

    def tearDown(self):
        self._td.cleanup()

    def write(self, name: str, data: bytes | str) -> Path:
        p = self.dir / name
        p.write_bytes(data if isinstance(data, bytes) else data.encode("utf-8"))
        return p


class TextFormatsTest(TmpCase):
    def test_utf8とcp932のテキスト(self):
        self.assertEqual(extract_text_file(self.write("a.txt", "日本語のメモ"))[0][1], "日本語のメモ")
        self.assertEqual(extract_text_file(self.write("b.txt", "日本語のメモ".encode("cp932")))[0][1], "日本語のメモ")

    def test_バイナリはテキストとして扱わない(self):
        with self.assertRaises(ExtractionError):
            extract_text_file(self.write("c.txt", b"\x00\x01\x02binary"))

    def test_htmlはscriptとstyleを除く(self):
        html = "<html><head><title>t</title></head><body><script>alert('x')</script><style>p{}</style><p>本文</p></body></html>"
        text = extract_html(self.write("d.html", html))[0][1]
        self.assertIn("本文", text)
        self.assertNotIn("alert", text)
        self.assertNotIn("p{}", text)


class SecretsTest(unittest.TestCase):
    def test_検出する(self):
        self.assertIsNotNone(find_secret("-----BEGIN RSA PRIVATE KEY-----\nabc"))
        self.assertIsNotNone(find_secret("key=AKIAABCDEFGHIJKLMNOP"))
        self.assertIsNotNone(find_secret("token ghp_" + "a" * 36))
        self.assertIsNotNone(find_secret("password = hunter2hunter2hunter2"))

    def test_普通の文章は検出しない(self):
        self.assertIsNone(find_secret("パスワードの管理方法について。ログインにはIDが要る。"))
        self.assertIsNone(find_secret("sk-は接頭辞の話"))


class RunnerTest(TmpCase):
    def test_テキストを抽出しキャッシュする(self):
        p = self.write("memo.txt", "検索したい本文です")
        r = extract_asset(p, self.cache)
        self.assertTrue(r.ok)
        self.assertEqual(r.sections, [("本文", "検索したい本文です")])
        self.assertEqual(len(list(self.cache.glob("*.json"))), 1)
        # 2回目はキャッシュから返る(結果が同じ)
        self.assertEqual(extract_asset(p, self.cache).sections, r.sections)

    def test_秘密情報を含む素材は止めて警告する(self):
        p = self.write("notes.txt", "メモ\nAWS: AKIAABCDEFGHIJKLMNOP\n")
        r = extract_asset(p, self.cache)
        self.assertFalse(r.ok)
        self.assertTrue(r.warn)
        self.assertIn("秘密情報", r.reason)
        self.assertEqual(r.sections, [])
        # 検出した文字列そのものは、理由に残さない
        self.assertNotIn("AKIA", r.reason)

    def test_未対応の形式と実行形式は抽出しない(self):
        for name in ("tool.exe", "run.sh", "image.png", "data.zip"):
            self.assertFalse(extract_asset(self.write(name, b"abc"), self.cache).ok, name)

    def test_大きすぎるファイルは飛ばす(self):
        p = self.write("big.txt", "a")
        original = limits.MAX_FILE_BYTES
        try:
            limits.MAX_FILE_BYTES = 0
            r = extract_asset(p, self.cache)
        finally:
            limits.MAX_FILE_BYTES = original
        self.assertFalse(r.ok)
        self.assertIn("大きすぎる", r.reason)

    def test_空のテキストは理由つきで成功扱い(self):
        r = extract_asset(self.write("empty.txt", "   \n"), self.cache)
        self.assertTrue(r.ok)
        self.assertEqual(r.sections, [])
        self.assertEqual(r.reason, "テキストなし")

    def test_壊れたpdfでも例外を出さない(self):
        r = extract_asset(self.write("broken.pdf", b"%PDF-1.4 not really a pdf"), self.cache)
        self.assertFalse(r.ok)


@unittest.skipUnless(HAS_DEFUSEDXML, "defusedxml未導入")
class OoxmlTest(TmpCase):
    def make_zip(self, name: str, files: dict[str, str]) -> Path:
        p = self.dir / name
        with zipfile.ZipFile(p, "w", zipfile.ZIP_DEFLATED) as z:
            for k, v in files.items():
                z.writestr(k, v)
        return p

    W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'

    def test_docx(self):
        xml = f'<w:document {self.W}><w:body><w:p><w:r><w:t>最初の段落</w:t></w:r></w:p><w:p><w:r><w:t>次の段落</w:t></w:r></w:p></w:body></w:document>'
        self.assertEqual(extract_docx(self.make_zip("a.docx", {"word/document.xml": xml}))[0][1], "最初の段落\n次の段落")

    def test_pptx(self):
        a = 'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"'
        files = {f"ppt/slides/slide{i}.xml": f"<p {a}><a:p><a:r><a:t>スライド{i}の文</a:t></a:r></a:p></p>" for i in (1, 2, 10)}
        labels = [s[0] for s in extract_pptx(self.make_zip("a.pptx", files))]
        self.assertEqual(labels, ["スライド1", "スライド2", "スライド3"])

    def test_xlsx(self):
        ns = 'xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"'
        files = {
            "xl/workbook.xml": f'<workbook {ns}><sheets><sheet name="売上"/></sheets></workbook>',
            "xl/sharedStrings.xml": f'<sst {ns}><si><t>商品</t></si><si><t>りんご</t></si></sst>',
            "xl/worksheets/sheet1.xml": f'<worksheet {ns}><sheetData><row><c t="s"><v>0</v></c><c><v>100</v></c></row><row><c t="s"><v>1</v></c><c><v>5</v></c></row></sheetData></worksheet>',
        }
        sec = extract_xlsx(self.make_zip("a.xlsx", files))
        self.assertEqual(sec[0][0], "売上")
        self.assertIn("りんご\t5", sec[0][1])

    def test_zip爆弾は展開せずに弾く(self):
        bomb = self.make_zip("bomb.docx", {"word/document.xml": "A" * (30 * 1024 * 1024)})
        with self.assertRaises(ExtractionError) as cm:
            extract_docx(bomb)
        self.assertIn("圧縮率", cm.exception.reason)

    def test_エンティティ展開攻撃を弾く(self):
        evil = ('<?xml version="1.0"?><!DOCTYPE lolz [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;">]>'
                f'<w:document {self.W}><w:body><w:p><w:r><w:t>&b;</w:t></w:r></w:p></w:body></w:document>')
        with self.assertRaises(ExtractionError):
            extract_docx(self.make_zip("evil.docx", {"word/document.xml": evil}))

    def test_壊れたzip(self):
        with self.assertRaises(ExtractionError):
            extract_docx(self.write("x.docx", b"not a zip"))


@unittest.skipUnless(HAS_PYPDF, "pypdf未導入")
class PdfTest(TmpCase):
    def test_暗号化pdfは抽出しない(self):
        from pypdf import PdfWriter

        w = PdfWriter()
        w.add_blank_page(100, 100)
        w.encrypt("pw")
        p = self.dir / "enc.pdf"
        with p.open("wb") as f:
            w.write(f)
        r = extract_asset(p, self.cache)
        self.assertFalse(r.ok)
        self.assertIn("暗号化", r.reason)

    def test_テキストの無いpdf(self):
        from pypdf import PdfWriter

        w = PdfWriter()
        w.add_blank_page(100, 100)
        p = self.dir / "blank.pdf"
        with p.open("wb") as f:
            w.write(f)
        r = extract_asset(p, self.cache)
        self.assertTrue(r.ok)
        self.assertEqual(r.reason, "テキストなし")


if __name__ == "__main__":
    unittest.main()
