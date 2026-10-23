"""段階5の比較用に、日本語の合成素材を足す(make_asset_fixtures.py の素材に加えて)。
使い方: python make_ja_fixtures.py <libraryのコピー> <msedge.exe>
"""
import subprocess
import sys
import zipfile
from pathlib import Path


def meta(d: Path, name: str, title: str, summary: str) -> None:
    (d / f"{name}.meta").write_text(
        f'---\ntitle: "{title}"\ntype: area\nproject: shiori\ntags: [詩織]\nsummary: "{summary}"\n'
        f"index: true\nstatus: new\n---\n\n## 中身\n{summary}\n",
        encoding="utf-8",
    )


def main() -> None:
    d = Path(sys.argv[1]) / "20-areas" / "shiori" / "files"
    d.mkdir(parents=True, exist_ok=True)
    edge = sys.argv[2]

    # 日本語のPDF(Edgeでhtmlを印刷する。フォントの埋め込みとToUnicodeを持つ、ふつうのPDFになる)
    html = d.parent / "_ja_pdf_src.html"
    html.write_text(
        "<html><head><meta charset='utf-8'><style>body{font-family:'Yu Gothic','Meiryo',sans-serif}</style></head><body>"
        "<h1>灯台守の業務日誌</h1><p>北の岬の灯台では、霧の夜にだけ琥珀色の補助灯をともす。補助灯の電球は、半年ごとに交換する。</p>"
        "<p>交換の記録は、青い表紙の台帳に書き、岬の郵便局へ写しを送る。</p>"
        "<div style='page-break-before:always'><h2>二枚目: 渡り鳥の観察</h2><p>秋の渡りの時期には、灯台の明かりを弱めて、鳥がぶつからないようにする。</p></div>"
        "</body></html>",
        encoding="utf-8",
    )
    pdf = d / "lighthouse.pdf"
    subprocess.run([edge, "--headless", "--disable-gpu", "--no-pdf-header-footer", f"--print-to-pdf={pdf}", html.as_uri()], check=True, timeout=120)
    html.unlink()
    meta(d, "lighthouse.pdf", "灯台の日誌PDF", "灯台の業務の記録")

    W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'
    with zipfile.ZipFile(d / "recipe.docx", "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr(
            "word/document.xml",
            f"<w:document {W}><w:body>"
            "<w:p><w:r><w:t>手順書: 苔玉の作り方</w:t></w:r></w:p>"
            "<w:p><w:r><w:t xml:space=\"preserve\">赤玉土とケト土を </w:t></w:r><w:r><w:t>三対七で混ぜ、</w:t></w:r><w:r><w:t>団子にする。</w:t></w:r></w:p>"
            "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>水やり</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>表面が乾いたら、バケツに沈めて泡が止まるまで待つ</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"
            "<w:p><w:r><w:t>注意 &amp; 補足: 直射日光に当てない。</w:t></w:r></w:p>"
            "</w:body></w:document>",
        )
    meta(d, "recipe.docx", "苔玉の手順書docx", "苔玉の作り方")

    P = 'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"'
    with zipfile.ZipFile(d / "deck.pptx", "w", zipfile.ZIP_DEFLATED) as z:
        for i, lines in enumerate([["文化祭の出し物", "天体観測カフェ"], ["当日の係", "望遠鏡の係は二年三組", "受付は生徒会"]] + [[f"予備のスライド{n}"] for n in range(3, 11)], 1):
            body = "".join(f"<a:p><a:r><a:t>{t}</a:t></a:r></a:p>" for t in lines)
            z.writestr(f"ppt/slides/slide{i}.xml", f"<p:sld {P}><p:cSld><p:spTree><p:sp><p:txBody>{body}</p:txBody></p:sp></p:spTree></p:cSld></p:sld>")
        z.writestr("ppt/slides/slide11.xml", f"<p:sld {P}><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>最後のスライド: 片付けは十七時から</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>")
    meta(d, "deck.pptx", "文化祭のスライドpptx", "文化祭の計画")

    NS = 'xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"'
    with zipfile.ZipFile(d / "stock.xlsx", "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("xl/workbook.xml", f'<workbook {NS}><sheets><sheet name="在庫"/><sheet name="発注"/></sheets></workbook>')
        z.writestr("xl/sharedStrings.xml", f"<sst {NS}><si><t>品名</t></si><si><r><t>星砂</t></r><r><t>の小瓶</t></r></si></sst>")
        z.writestr("xl/worksheets/sheet1.xml", f'<worksheet {NS}><sheetData><row><c t="s"><v>0</v></c><c t="inlineStr"><is><t>数量</t></is></c></row><row><c t="s"><v>1</v></c><c><v>12</v></c></row></sheetData></worksheet>')
        z.writestr("xl/worksheets/sheet2.xml", f'<worksheet {NS}><sheetData><row><c t="inlineStr"><is><t>発注先は浜辺の雑貨屋</t></is></c></row></sheetData></worksheet>')
    meta(d, "stock.xlsx", "在庫表xlsx", "雑貨の在庫")

    (d / "old.txt").write_bytes("古い日記: 縁側で線香花火をした夏。".encode("cp932"))
    meta(d, "old.txt", "古い日記txt", "Shift_JISの日記")
    (d / "list.csv").write_bytes("名前,好物\r\nたま,煮干し\r\n".encode("utf-16"))
    meta(d, "list.csv", "猫の好物csv", "UTF-16の表")
    (d / "entity.html").write_text("<html><head><title>題は捨てる</title></head><body><h1>月見団子の数</h1><p>十五夜には&nbsp;十五個、十三夜には&#21313;三個を供える。</p><br/><li>すすき</li></body></html>", encoding="utf-8")
    meta(d, "entity.html", "月見の作法html", "お月見の記事")


if __name__ == "__main__":
    main()
