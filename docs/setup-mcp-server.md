# mcp_server セットアップ手順(Ver2.0)

`system/src-tauri/src/bin/mcp_server.rs` は、詩織のライブラリ(`library/`)を検索専用で参照するstdioベースのMCPサーバー。会話UI本体(`shiori-folio`)とは別バイナリで、Claude Code・Claude Desktop等のAIエージェントから直接起動される想定。

書き込みツールは持たない。書き込みは `shiori-save` CLI([setup-shiori-save.md](setup-shiori-save.md)参照、未作成の場合はこの文書と対で整備すること)経由で行う。

## ビルド

`system/src-tauri/` の通常のCargoパッケージに `[[bin]]` として同居しており、`src/bin/*.rs` の自動検出により追加設定なしでビルドされる。

```bash
cd system/src-tauri
cargo build --release --bin mcp_server
```

生成物:
- Windows: `target/release/mcp_server.exe`
- macOS/Linux: `target/release/mcp_server`

> `npm run tauri build`(`tauri build`経由のビルド)は会話UI本体(`shiori-folio`)のみを対象とし、`mcp_server`・`shiori-save`はビルドしない。この2つは上記のように個別に`cargo build --release --bin <name>`する必要がある。
>
> **`cargo build --release --bins`(全バイナリ一括)は使わないこと。** `shiori-folio`も対象に含まれてしまい、`tauri build`が付与する`custom-protocol`フィーチャーフラグ無しで再ビルドされる結果、フロントエンド資産が埋め込まれず「詩織を起動してもホーム画面に到達できず、埋め込みビルド時のみ動くページの読み込みに失敗する」不具合が起きる(2026-08-17、Windows実機で`--bins`実行直後にLLM/フロントエンドが一切起動しなくなる形で再現・特定。`shiori-folio.exe`のファイルサイズが約20MB→約14MBに縮んでいたら壊れているサイン)。もし誤って上書きしてしまった場合は`npm run tauri build`を再実行すれば直る。

## 前提

- `library/`が実際に存在すること(`project_root()`の親、または`portable/`直下)。無い場合は「詩織のSSDが接続されていません」エラーを返す
- `library/_system/tier1-profile.md`が存在すること(`get_context_profile`ツールが読む)。存在しない場合はこのツール呼び出し時にエラーになる
- 検索実行時、embedding用llama-server・RAG Pythonサーバーが未起動なら`mcp_server`自身が共有デーモンとして起動を試みる(`shared_daemon::ensure_daemon_running`)。会話UIが起動していなくても単体で検索が完結する

## Claude Desktop への登録

設定ファイルの場所はOSごとに異なる。

| OS | パス |
|---|---|
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |

`mcpServers`に以下のように追加する(パスは環境に合わせて絶対パスで指定すること)。

```jsonc
// Windows例
{
  "mcpServers": {
    "shiori-library": {
      "command": "C:\\Users\\<user>\\Documents\\myProgramdata\\Shiori-folio\\system\\src-tauri\\target\\release\\mcp_server.exe"
    }
  }
}
```

```jsonc
// macOS例
{
  "mcpServers": {
    "shiori-library": {
      "command": "/path/to/Shiori-folio/system/src-tauri/target/release/mcp_server"
    }
  }
}
```

設定後、Claude Desktopを再起動すると `get_context_profile` / `search_library` の2ツールが利用可能になる。

## Claude Code への登録

```bash
claude mcp add shiori-library -- /path/to/mcp_server(.exe)
```

もしくは `.mcp.json`(プロジェクトスコープ)に同様の`command`を追加する。

## 動作確認

stdioベースのため、単体で起動してもすぐには終了せず標準入力を待ち続ける(正常な挙動)。JSON-RPCの`tools/list`リクエストを1行渡して応答が返るかで疎通確認できる。

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | ./mcp_server
```

`get_context_profile`・`search_library`の2ツールがリストされれば正常。
