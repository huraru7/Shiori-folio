# shiori-save セットアップ手順(Ver2.0)

`system/src-tauri/src/bin/shiori_save.rs` は、`library/`への保存を担うCLI。配置先フォルダの判定・タグ/プロジェクトの正規化をLLMの裁量に任せず、決定的なロジックとして実装している(配置ルールは`library/_system/CLAUDE.md`参照)。

Claude Code等のAIエージェントは、frontmatter(title/type/tags/project/author)付きのMarkdownファイルを任意の場所に書き出した後、このCLIに渡すことで`library/`配下の正しい場所へ保存できる。

## ビルド

`mcp_server`と同じく`src/bin/*.rs`の自動検出でビルドされる。`npm run tauri build`では生成されないため、個別にビルドすること。

```bash
cd system/src-tauri
cargo build --release --bin shiori_save
```

生成物は`target/release/shiori_save`(Windowsは`shiori_save.exe`)。ポータブル版(`portable/bin/<os>/`)には`shiori-save`(ハイフン区切り)にリネームして同梱される(`scripts/build-portable.ps1` / `build-portable.sh`参照)。

> **`cargo build --release --bins`(全バイナリ一括)は使わないこと。** 詳細は[setup-mcp-server.md](setup-mcp-server.md)の同名の注記を参照(会話UI本体`shiori-folio`を巻き込んで壊してしまう)。

## 使用法

```bash
shiori-save <mdファイルのパス>
```

対象ファイルにfrontmatter(title/type/tags/project/author)が付与済みであることが前提。実行後、対象ファイルは`library/`配下の決定先へ**移動**される(コピーではなく移動、元の場所には残らない)。

- `type`が`decision`/`project-log`/`task`/`principle`/`insight`/`experience`/`glossary`/`reference`のいずれでもない、または`title`/`tags`が空など不備がある場合は、保存自体は諦めず`00-inbox/`へ`reason`フィールド付きで退避する
- タグ・プロジェクトは`library/_system/tags.yaml`・`projects.yaml`と照合し、未登録のものは`status: pending`で自己登録される(Ver2.0 Phase 8の承認UIで後から承認・却下・保留できる)

## 動作確認

```bash
echo '---
title: テスト
type: insight
tags: [動作確認]
author: human
---
本文。' > /tmp/test.md

shiori-save /tmp/test.md
```

`library/30-knowledge/general/`(または該当カテゴリ)に保存されれば正常。
