"""素材抽出(詩織Ver3.8)の評価用の素材を作る。実機のlibraryには置かないこと。

使い方: python eval/make_asset_fixtures.py <libraryのコピー>
  → <libraryのコピー>/20-areas/shiori/files/ に、各形式の素材と.metaを作る。
    そのlibraryを指す検証用のRAGサーバーを立て、asset_dataset.jsonで測る(README相当は下記)。
    1. 本番のlibraryをコピーする(rsync -a library/ /tmp/x/library/)
    2. このスクリプトで素材を足す
    3. /tmp/x/{config.json, library, services/rag} の形で、別ポートのRAGサーバーを
       SHIORI_VECTORDB_DIR=/tmp/x/vectordb で起動する
    4. python eval/run_search_eval.py --tag assets --dataset eval/asset_dataset.json --base http://127.0.0.1:<port>
依存: pypdf(暗号化PDFの作成に使う)。
"""
from __future__ import annotations

import sys
import zipfile
from pathlib import Path

from pypdf import PdfWriter


def meta(d: Path, name: str, title: str, summary: str, extra: str = "") -> None:
    (d / f"{name}.meta").write_text(
        f'---\ntitle: "{title}"\ntype: area\nproject: shiori\ntags: [詩織]\nsummary: "{summary}"\n'
        f"index: true\nstatus: new\n{extra}---\n\n## 中身\n{summary}\n",
        encoding="utf-8",
    )


def pdf_with_text(path: Path, text: str) -> None:
    stream = f"BT /F1 24 Tf 50 700 Td ({text}) Tj ET".encode()
    objs = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    out = b"%PDF-1.4\n"
    offs = []
    for i, o in enumerate(objs, 1):
        offs.append(len(out))
        out += b"%d 0 obj\n" % i + o + b"\nendobj\n"
    x = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
    for o in offs:
        out += b"%010d 00000 n \n" % o
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, x)
    path.write_bytes(out)


def main() -> None:
    d = Path(sys.argv[1]) / "20-areas" / "shiori" / "files"
    d.mkdir(parents=True, exist_ok=True)
    W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'
    NS = 'xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"'

    (d / "notes.txt").write_text("宇宙ステーションの冷却ポンプは、毎週火曜に点検する。ゼブラトークンという合言葉は議事録にだけ出てくる。", encoding="utf-8")
    meta(d, "notes.txt", "議事録テキスト", "会議のメモ")
    pdf_with_text(d / "report.pdf", "QUASARFLUX calibration procedure")
    meta(d, "report.pdf", "校正手順書PDF", "装置の校正手順")
    with zipfile.ZipFile(d / "spec.docx", "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("word/document.xml", f"<w:document {W}><w:body><w:p><w:r><w:t>仕様書: 星間通信モジュールの帯域は、コスモビームで決まる。</w:t></w:r></w:p></w:body></w:document>")
    meta(d, "spec.docx", "仕様書docx", "通信モジュールの仕様")
    with zipfile.ZipFile(d / "sales.xlsx", "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("xl/workbook.xml", f'<workbook {NS}><sheets><sheet name="月次"/></sheets></workbook>')
        z.writestr("xl/sharedStrings.xml", f"<sst {NS}><si><t>ムーンベリー</t></si></sst>")
        z.writestr("xl/worksheets/sheet1.xml", f'<worksheet {NS}><sheetData><row><c t="s"><v>0</v></c><c><v>42</v></c></row></sheetData></worksheet>')
    meta(d, "sales.xlsx", "売上表xlsx", "月次の売上")
    (d / "page.html").write_text('<html><body><script>var x="SCRIPTONLYWORD"</script><p>ウェブ記事: 海底ケーブルの保守は、ネプチューン計画と呼ばれる。</p></body></html>', encoding="utf-8")
    meta(d, "page.html", "ウェブ記事html", "記事の保存")
    # 抽出されないはずの素材
    (d / "secret.txt").write_text("接続メモ\nAWSキー AKIAABCDEFGHIJKLMNOP を使う。ヴァルキリー計画。", encoding="utf-8")
    meta(d, "secret.txt", "接続メモ", "秘密を含むメモ")
    (d / "private.txt").write_text("機密のはずの本文: オプトアウトワード・ゼロニウム", encoding="utf-8")
    meta(d, "private.txt", "機密テキスト", "抽出しない素材", "extract: false\n")
    with zipfile.ZipFile(d / "bomb.docx", "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("word/document.xml", "A" * (30 * 1024 * 1024))
    meta(d, "bomb.docx", "爆弾docx", "圧縮率の異常なzip")
    w = PdfWriter()
    w.add_blank_page(100, 100)
    w.encrypt("pw")
    with (d / "enc.pdf").open("wb") as f:
        w.write(f)
    meta(d, "enc.pdf", "暗号化PDF", "パスワード付き")
    w = PdfWriter()
    w.add_blank_page(100, 100)
    with (d / "scan.pdf").open("wb") as f:
        w.write(f)
    meta(d, "scan.pdf", "スキャンPDF", "テキストなし")
    (d / "broken.pdf").write_bytes(b"%PDF-1.4 broken")
    meta(d, "broken.pdf", "壊れたPDF", "壊れている")
    (d / "tool.sh").write_text("echo hi")
    meta(d, "tool.sh", "実行形式", "シェルスクリプト")


if __name__ == "__main__":
    main()
