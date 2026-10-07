"""形式ごとの抽出関数と、拡張子の登録表(REGISTRY)。

抽出関数は (path: Path) -> list[Section] を返す。Sectionは(見出し, 本文)。
新しい形式は、抽出関数を1つ書いてREGISTRYに拡張子を足すだけで追加できる。
読むだけで、マクロ・JavaScript・埋め込みファイルは実行も展開もしない。
"""
from __future__ import annotations

import re
import zipfile
from html.parser import HTMLParser
from pathlib import Path
from typing import Callable

from . import limits

Section = tuple[str, str]


class ExtractionError(Exception):
    """抽出できない理由が分かっている失敗。reasonは記録・警告にそのまま出す。"""

    def __init__(self, reason: str, detail: str = ""):
        super().__init__(f"{reason}: {detail}" if detail else reason)
        self.reason = reason


# ---------- テキスト系 ----------

_TEXT_ENCODINGS = ("utf-8-sig", "cp932", "utf-16")


def _decode_text(data: bytes) -> str:
    if b"\x00" in data[:4096] and not data.startswith((b"\xff\xfe", b"\xfe\xff")):
        raise ExtractionError("テキストではない(バイナリ)")
    for enc in _TEXT_ENCODINGS:
        try:
            return data.decode(enc)
        except UnicodeDecodeError:
            continue
    return data.decode("utf-8", errors="replace")


def extract_text_file(path: Path) -> list[Section]:
    return [("本文", _decode_text(path.read_bytes()))]


class _HtmlText(HTMLParser):
    _SKIP = {"script", "style", "noscript", "template", "head"}
    _BLOCK = {"p", "div", "br", "li", "tr", "h1", "h2", "h3", "h4", "h5", "h6", "section", "article", "table"}

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []
        self._skip_depth = 0

    def handle_starttag(self, tag, attrs):
        if tag in self._SKIP:
            self._skip_depth += 1
        elif tag in self._BLOCK:
            self.parts.append("\n")

    def handle_endtag(self, tag):
        if tag in self._SKIP and self._skip_depth:
            self._skip_depth -= 1
        elif tag in self._BLOCK:
            self.parts.append("\n")

    def handle_data(self, data):
        if not self._skip_depth:
            self.parts.append(data)


def extract_html(path: Path) -> list[Section]:
    parser = _HtmlText()
    parser.feed(_decode_text(path.read_bytes()))
    text = re.sub(r"\n\s*\n+", "\n\n", "".join(parser.parts)).strip()
    return [("本文", text)]


# ---------- PDF ----------


def extract_pdf(path: Path) -> list[Section]:
    try:
        from pypdf import PdfReader
        from pypdf.errors import PdfReadError
    except ImportError as e:
        raise ExtractionError("抽出器が未導入(pypdf)") from e
    try:
        reader = PdfReader(str(path), strict=False)
        if reader.is_encrypted:
            # パスワードの試行はしない(空パスワードで開けるものも含め、暗号化PDFは扱わない)。
            raise ExtractionError("暗号化されたPDF")
        sections: list[Section] = []
        for i, page in enumerate(reader.pages):
            if i >= limits.MAX_PAGES:
                break
            text = (page.extract_text() or "").strip()
            if text:
                sections.append((f"p.{i + 1}", text))
        return sections
    except ExtractionError:
        raise
    except PdfReadError as e:
        raise ExtractionError("壊れたPDF", str(e)[:100]) from e


# ---------- OOXML(docx / pptx / xlsx。中身はzip+XML) ----------


def _open_zip_safely(path: Path) -> zipfile.ZipFile:
    """zip爆弾(展開後が巨大・圧縮率が異常)を、展開する前に弾いてから開く。"""
    try:
        zf = zipfile.ZipFile(path)
    except zipfile.BadZipFile as e:
        raise ExtractionError("壊れたファイル(zipとして読めない)") from e
    infos = zf.infolist()
    if len(infos) > limits.ZIP_MAX_MEMBERS:
        raise ExtractionError("zipの中のファイルが多すぎる")
    total = 0
    for info in infos:
        total += info.file_size
        if info.file_size > limits.ZIP_MAX_MEMBER_BYTES or total > limits.ZIP_MAX_TOTAL_BYTES:
            raise ExtractionError("zipの展開サイズが大きすぎる")
        if info.compress_size and info.file_size / info.compress_size > limits.ZIP_MAX_RATIO:
            raise ExtractionError("zipの圧縮率が異常(zip爆弾の疑い)")
    return zf


def _parse_xml(zf: zipfile.ZipFile, name: str):
    try:
        from defusedxml import ElementTree
    except ImportError as e:
        raise ExtractionError("抽出器が未導入(defusedxml)") from e
    try:
        return ElementTree.fromstring(zf.read(name))
    except KeyError as e:
        raise ExtractionError("必要な部品が無い", name) from e
    except ElementTree.ParseError as e:
        raise ExtractionError("壊れたXML", name) from e
    except Exception as e:  # defusedxmlの禁止(エンティティ展開等)も含む
        raise ExtractionError("不正なXML", f"{name}: {type(e).__name__}") from e


def _local(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _numbered(zf: zipfile.ZipFile, prefix: str) -> list[str]:
    names = [n for n in zf.namelist() if n.startswith(prefix) and n.endswith(".xml")]
    return sorted(names, key=lambda n: int(re.search(r"(\d+)\.xml$", n).group(1)) if re.search(r"(\d+)\.xml$", n) else 0)


def extract_docx(path: Path) -> list[Section]:
    zf = _open_zip_safely(path)
    root = _parse_xml(zf, "word/document.xml")
    paragraphs = []
    for p in root.iter():
        if _local(p.tag) == "p":
            text = "".join(t.text or "" for t in p.iter() if _local(t.tag) == "t")
            if text.strip():
                paragraphs.append(text)
    return [("本文", "\n".join(paragraphs))]


def extract_pptx(path: Path) -> list[Section]:
    zf = _open_zip_safely(path)
    sections: list[Section] = []
    for i, name in enumerate(_numbered(zf, "ppt/slides/slide")):
        if i >= limits.MAX_PAGES:
            break
        root = _parse_xml(zf, name)
        lines = []
        for p in root.iter():
            if _local(p.tag) == "p":
                text = "".join(t.text or "" for t in p.iter() if _local(t.tag) == "t")
                if text.strip():
                    lines.append(text)
        if lines:
            sections.append((f"スライド{i + 1}", "\n".join(lines)))
    return sections


def extract_xlsx(path: Path) -> list[Section]:
    zf = _open_zip_safely(path)
    shared: list[str] = []
    if "xl/sharedStrings.xml" in zf.namelist():
        for si in _parse_xml(zf, "xl/sharedStrings.xml"):
            shared.append("".join(t.text or "" for t in si.iter() if _local(t.tag) == "t"))
    sheet_names: list[str] = []
    if "xl/workbook.xml" in zf.namelist():
        wb = _parse_xml(zf, "xl/workbook.xml")
        sheet_names = [s.get("name", "") for s in wb.iter() if _local(s.tag) == "sheet"]
    sections: list[Section] = []
    for i, name in enumerate(_numbered(zf, "xl/worksheets/sheet")):
        if i >= limits.MAX_PAGES:
            break
        root = _parse_xml(zf, name)
        rows = []
        for row in root.iter():
            if _local(row.tag) != "row":
                continue
            cells = []
            for c in row:
                if _local(c.tag) != "c":
                    continue
                v = next((x for x in c if _local(x.tag) == "v"), None)
                inline = "".join(t.text or "" for t in c.iter() if _local(t.tag) == "t")
                if c.get("t") == "s" and v is not None and v.text and v.text.isdigit() and int(v.text) < len(shared):
                    cells.append(shared[int(v.text)])
                elif c.get("t") == "inlineStr":
                    cells.append(inline)
                elif v is not None and v.text:
                    cells.append(v.text)
            if cells:
                rows.append("\t".join(cells))
        if rows:
            label = sheet_names[i] if i < len(sheet_names) and sheet_names[i] else f"シート{i + 1}"
            sections.append((label, "\n".join(rows)))
    return sections


# 拡張子(小文字、ドットなし) → 抽出関数。形式を増やすときは、ここに足す。
REGISTRY: dict[str, Callable[[Path], list[Section]]] = {
    "pdf": extract_pdf,
    "docx": extract_docx,
    "pptx": extract_pptx,
    "xlsx": extract_xlsx,
    "html": extract_html,
    "htm": extract_html,
    **{ext: extract_text_file for ext in ("txt", "text", "log", "csv", "tsv", "json", "yaml", "yml", "xml", "ini", "toml", "rst", "md")},
}


def supported(path: Path) -> bool:
    return path.suffix.lower().lstrip(".") in REGISTRY
