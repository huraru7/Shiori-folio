# shiori-rag の比較の道具(Ver6.0 段階5)

Python版(`services/rag`)とRust版(`shiori-rag`)を並べて比べるためのスクリプト。実機の索引やlibraryには書かない(libraryは読むだけ、素材の比較はlibraryのコピーで行う)。

| ファイル | 使い方 |
|---|---|
| `../../rag/eval/run_search_eval.py` | 評価セットを、それぞれのサーバーに `--base http://127.0.0.1:<port>` で通す |
| `cmp_eval.py` | 上の結果のJSONを2つ並べ、質問ごとの順位の違いを出す |
| `cmp_api.py` | 同じ要求を2つのサーバーに送り、応答の形・並び・点数・距離・時間を比べる |
| `py_chunks.py`・`cmp_chunks.py` | チャンク分けとfrontmatterの値を比べる(Rust版は `shiori-rag dump-chunks <library>`) |
| `hnsw_check.py` | ChromaDBの近似の索引が取りこぼす候補を数える(索引のコピーに対して使う) |
| `make_asset_fixtures_ja.py`・`asset_dataset_ja.json` | `../../rag/eval/make_asset_fixtures.py` の素材に、日本語の素材(PDFはEdgeで印刷して作る)を足し、15問で測る |

結果は、詩織のlibraryの企画書 `plan-shiori-ver6-s5-rag-20261023` の「結果」に書いてある。
