# 入口(詳細は詩織のlibraryが正本)

このリポジトリは詩織(project id: `shiori`)の開発機。AIの種類(Claude Code、Codexなど)を問わず、次に従うこと。

作業を始めるときは、まず詩織の実機のlibraryにある次の2つを読む。

1. `40-profile/memory-global.md`(ふらるとAI全体の約束事と、いま有効な状態)
2. `20-areas/shiori/memory.md`(詩織の、いま有効な状態・やること・申し送り)

- 実機は外付けSSD `ShioriFolio` の `portable/`。Windowsは `E:\portable\library\`、Macは `/Volumes/ShioriFolio/portable/library/`。このリポジトリの `library/` は開発用の空のもので、読み書きに使わない。
- SSDが接続されていないときは、黙って別の動作をせず、その旨をユーザーに伝える。
- `kind: memory` の更新は `shiori-save --expect-updated "<読んだupdated>"` 経由のみ。手順は `library/_system/CLAUDE.md` の「共通記憶MD」。
- skillの手順の本体は `30-resources/skills/` にある(例: `work-start.md`、`git-workflow.md`)。
- 配置と検証の注意点は、libraryの `deploy-and-verify-pitfalls-20261008` を先に読む。

# 応答言語(SSDが読めないときの最低限)

常に日本語で応答すること。コード内のコメントも日本語で書く。
