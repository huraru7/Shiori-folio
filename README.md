# 詩織 (Shiori)

個人用AIアシスタント「詩織」のデスクトップアプリ本体。Tauri(Rust) + React製で、会話(LLM/STT/TTS)とセカンドブレイン(RAGによる長期記憶)の両方をローカル完結で動かすことを目指したプロジェクト。

## 構成

- **基盤**: Tauri(Rust) + React
- **会話**: llama.cpp(LLM) / whisper.cpp(STT) / piper-plus(TTS)
- **記憶**: ChromaDB + reranker(RAG)によるセカンドブレイン(`library/`配下のMarkdownを検索・参照)
- **Claude Code連携**: MCPサーバー経由でセカンドブレインの検索・保存が可能

詳細な内部構成は [docs/shiori-reference.html](docs/shiori-reference.html) を参照。

## セットアップ

このリポジトリにはGit管理外のものが2種類ある。

- `models/` 配下のLLM/STT/TTSモデルファイル(`.gguf`/`.bin`/`.onnx`)は容量が大きいため対象外。各自でダウンロードして配置すること
- `third_party/` 配下(llama.cpp・whisper.cpp・piper-plus)はビルド済みバイナリの実クローンのため対象外。clone・ビルド手順とWindows環境で動作確認済みの固定バージョンは [docs/setup-third-party.md](docs/setup-third-party.md) を参照

開発サーバーの起動やビルド手順は通常のTauri + Viteプロジェクトと同様(`npm install` → `npm run tauri dev` / `npm run tauri build`)。

## ライセンス

[MIT License](LICENSE)
