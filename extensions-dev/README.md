# 試しの拡張機能(詩織Ver6.0の段階1・2)

段階1の受け入れ基準を確かめるための拡張機能の元。確かめるときに、実機の `portable/extensions/` へ写す(実機に置く前に確認を取る)。

- `hello-ui`: 画面の部品の拡張機能(段階2からは列の小道具。自己確認の起動で16項目を自動で試す)
- `hello-screen`: 画面の部品の拡張機能(段階2、列の画面に増えることを確かめる)
- `hello-program`: 別のプログラムの拡張機能(Rust)。`greet` と `crash`(わざと落ちる)を提供する。写す前に、フォルダの中で `cargo build --release` し、`target/release/hello-program(.exe)` を `bin/` に写す(`bin/` と `target/` はgitに入れない)

## 自分の確かめ(説明書の `selftest`、段階6)

画面の部品の拡張機能は、自分の確かめを持てる。持つときは、説明書に `"selftest": true` と書く(省略は `false`)。

- 詩織の自己確認の起動(`--self-test`)は、iframeに `{shiori: 1, selftestRun: true}` を送る。
- 受けたら、まず `{shiori: 1, selftest: {started: true}}` を返し、確かめが終わったら `{shiori: 1, selftest: {ok, ng}}` を返す(例は `hello-ui/app.js`)。
- `selftest: true` なのに応えなければ、自己確認は「応えない」でNGになる(スクリプトが読めていないときなどに気づくため)。
- この項目を知らない古い本体(段階6より前)は、説明書を `EXT-002` で断る。古い本体と共有する実機の `portable/extensions/` へ写すときは気をつける。

詳しくは企画書 `plan-shiori-ver6-s1-foundation-20261023` と `plan-shiori-ver6-s6-doctor-20261024`(詩織のlibrary)。
