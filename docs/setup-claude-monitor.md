# Claudeモニター セットアップ手順(Ver3.7)

動いているClaude Codeのセッションを、詩織の「Claudeモニター」(Dockの📡)に表示する。
Claude Code側のフックが `portable/scripts/claude-report.js` を呼び、
`portable/data/claude-status/<session_id>.json` に状況を書く。詩織はそれを読むだけ。

- 保存する項目: 状態・プロジェクト名・セッションタイトル・Mac/Windowsの別・Claude Codeのpid・更新時刻。プロンプト・応答の中身は保存しない。
- SSDが未接続でも、Claudeの作業は止まらない(スクリプトは何もせず正常終了する)。
- 使用量(5時間/週次)は表示しない。使用量はClaude Codeのステータスラインの入力にしかなく、デスクトップアプリのCodeタブでは呼ばれなかったため(2026-10-07)。

## 前提

- Node.jsが使えること(`node -v`)。
- `portable/scripts/claude-report.js` が存在すること(`build-portable.sh`/`.ps1` がコピーする。ソースは `system/scripts/claude-report.js`)。

## 設定(各PCの `~/.claude/settings.json`)

Mac・Windowsで設定ファイルは別々なので、使うPCごとに入れる。既存の `hooks` がある場合は、各イベントの配列に追記する(上書きしない)。

### Mac

```json
{
  "hooks": {
    "SessionStart":     [{ "hooks": [{ "type": "command", "timeout": 10, "command": "test -f \"/Volumes/ShioriFolio/portable/scripts/claude-report.js\" && node \"/Volumes/ShioriFolio/portable/scripts/claude-report.js\" || exit 0" }] }],
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "timeout": 10, "command": "(同上)" }] }],
    "Notification":     [{ "hooks": [{ "type": "command", "timeout": 10, "command": "(同上)" }] }],
    "Stop":             [{ "hooks": [{ "type": "command", "timeout": 10, "command": "(同上)" }] }],
    "SessionEnd":       [{ "hooks": [{ "type": "command", "timeout": 10, "command": "(同上)" }] }],
    "PostToolUse":      [{ "hooks": [{ "type": "command", "timeout": 10, "async": true, "command": "(同上)" }] }]
  }
}
```

`(同上)` は、`SessionStart` と同じコマンド文字列に置き換える。

### Windows

`command` のパスを、実機SSDのドライブ文字に合わせる(例: `E:\portable\scripts\claude-report.js`)。
Claude Codeのフックのシェルは環境によって異なる(Git Bash / PowerShell)。次のどちらかで入れる。

- Git Bash の場合: Macと同じ `test -f "E:/portable/scripts/claude-report.js" && node "E:/portable/scripts/claude-report.js" || exit 0`
- PowerShell の場合: `if (Test-Path "E:\portable\scripts\claude-report.js") { node "E:\portable\scripts\claude-report.js" }; exit 0`

Windowsでの動作は未確認(Macのみ確認済み)。入れたら、新しいセッションを開いて `portable/data/claude-status/` にJSONができるかを確かめる。

## 確認

1. 新しいセッションを開き、1言送る。
2. `portable/data/claude-status/` に `<session_id>.json` ができ、`state` が `working` → `idle` と変わる。
3. 詩織のDockの📡からClaudeモニターを開くと、同じ状態が一覧に出る。

## 仕様メモ

- 状態: `SessionStart`→待機、`UserPromptSubmit`→作業中、`Notification`→入力待ち、`Stop`→待機、`SessionEnd`→終了。`PostToolUse` は更新時刻を進め、入力待ち・待機から「作業中」へ戻す(ただしStop直後の5秒は待機のまま)。
- 詩織側の死活判定: フックは生存確認の通信をせず、長いビルドや長い推論の間は無音になる(Claudeの作業は10分を超えることがある)ため、時間では判定しない。スクリプトが親プロセスをたどって、起動元のClaude Codeのpidを記録する(Macは`ps`、Windowsは`Get-CimInstance`。Windowsは未確認)。詩織は、同じPCのセッションのpidが生きているか(プロセス名が`claude`か)を見る。生きていれば無音が何時間続いても状態を信じる。消えているのに終了の報告が無いものは「終了」扱いで、5分で一覧から外す。
- 別のPCのセッションと、pidが取れなかったものは、確かめようがない。更新が1時間止まった作業中・入力待ちを「応答なし」、状態を問わず12時間止まったものは一覧から外す。
- ファイルの掃除は報告スクリプトが行う(`SessionStart` のとき、終了後1時間・更新が止まって24時間のものを削除)。
- プロジェクト名は、フックに渡る `CLAUDE_PROJECT_DIR`(無ければ最初に見えた作業フォルダ)の末尾名。
