# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## セットアップ

このリポジトリにはGit管理外のものが2種類ある。

- `models/` 配下のLLM/STT/TTSモデルファイル(`.gguf`/`.bin`/`.onnx`)は容量が大きいため対象外。各自でダウンロードして配置すること
- `third_party/` 配下(llama.cpp・whisper.cpp・piper-plus)はビルド済みバイナリの実クローンのため対象外。clone・ビルド手順とWindows環境で動作確認済みの固定バージョンは [docs/setup-third-party.md](docs/setup-third-party.md) を参照
