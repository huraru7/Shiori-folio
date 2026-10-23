# 試しの拡張機能(詩織Ver6.0の段階1)

段階1の受け入れ基準を確かめるための拡張機能の元。確かめるときに、実機の `portable/extensions/` へ写す(実機に置く前に確認を取る)。

- `hello-ui`: 画面の部品の拡張機能
- `hello-program`: 別のプログラムの拡張機能(Rust)。`greet` と `crash`(わざと落ちる)を提供する。写す前に、フォルダの中で `cargo build --release` し、`target/release/hello-program(.exe)` を `bin/` に写す(`bin/` と `target/` はgitに入れない)

詳しくは企画書 `plan-shiori-ver6-s1-foundation-20261023`(詩織のlibrary)。
