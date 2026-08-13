# third_party/ セットアップ手順

`system/third_party/` 配下(llama.cpp・whisper.cpp・piper-plus)は各エンジンの実クローン(独自の`.git`を持つ)であり、`.gitignore`で丸ごとGit管理外にしている。理由:

- サブモジュール化していないため、そのまま`git add`すると「埋め込みリポジトリ(gitlink)」扱いになり、`.gitmodules`なしでは中身が空扱いで消失する事故につながる
- `build/`配下はWindows+CUDA向けにビルドしたバイナリで、他OS(macOS等)に持ち込んでも動作しない

そのため、**Windows開発環境で実際にビルド・動作を確認しているコミットを固定**し、新しい環境ではこの手順で再現する。「最新版をclone」はしないこと(上流の破壊的変更で動かなくなるリスクがあるため)。

## 固定バージョン一覧(2026-08-13 時点でWindows環境にて動作確認済み)

| エンジン | 用途 | upstream | 固定コミット | タグ/バージョン |
|---|---|---|---|---|
| llama.cpp | LLM推論(`llama-server`) | https://github.com/ggml-org/llama.cpp.git | `b06aa774c03dbbb624e726664b714a57d1f49815` | (タグなし、上記ハッシュで固定) |
| whisper.cpp | STT(`whisper-server`) | https://github.com/ggml-org/whisper.cpp.git | `306c88f4d1286aec1bf96e544632897886af5501` | `v1.9.2` |
| piper-plus | TTS(`piper-plus-cli`) | https://github.com/ayutaz/piper-plus.git | `d2140faaa86ca3042a8c12c3dc4d2275ed4c5a23` | (タグなし、上記ハッシュで固定) |

> piper-plusのみ、Windows環境で`dotnet restore`実行時に`src/csharp/**/packages.lock.json`がローカルで微差分を生んでいたことを確認済み(C#ランタイム側の話で、実際にアプリが使う`piper-plus-cli`はRust実装のため動作には影響しない)。気になる場合は`git checkout -- src/csharp`でクリーンな状態に戻せる。

## clone手順

```bash
cd system/third_party

git clone https://github.com/ggml-org/llama.cpp.git
cd llama.cpp && git checkout b06aa774c03dbbb624e726664b714a57d1f49815 && cd ..

git clone https://github.com/ggml-org/whisper.cpp.git
cd whisper.cpp && git checkout 306c88f4d1286aec1bf96e544632897886af5501 && cd ..

git clone https://github.com/ayutaz/piper-plus.git
cd piper-plus && git checkout d2140faaa86ca3042a8c12c3dc4d2275ed4c5a23 && cd ..
```

## ビルド手順

### llama.cpp / whisper.cpp

Windows環境ではCUDAビルド(Visual Studio 17 2022、`CMAKE_CUDA_ARCHITECTURES=89`)で確認済み。アプリは以下のパスの実行ファイルを直接呼び出す(`src-tauri/src/lib.rs`参照)。

- `third_party/llama.cpp/build/bin/Release/llama-server.exe`
- `third_party/whisper.cpp/build/bin/Release/whisper-server.exe`

```bash
# 例(Windows + CUDA、GPUアーキテクチャは環境のGPUに合わせて変更)
cmake -B build -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=89
cmake --build build --config Release
```

**macOSへの移行時の注意**: 上記はWindows+CUDA固有のビルド設定であり、そのままでは使えない。macOSでは`-DGGML_METAL=ON`(Apple Silicon)等、Metalバックエンドでのビルドに置き換える必要がある。コミットを固定しているのはエンジンの「バージョン(挙動)」を再現するためであり、ビルドフラグそのものはOSごとに読み替える。

### piper-plus

アプリが実際に呼び出すのはRust実装の`piper-plus-cli`(`third_party/piper-plus/src/rust/`のCargoワークスペース)。

- `third_party/piper-plus/src/rust/target/release/piper-plus-cli.exe`

```bash
cd third_party/piper-plus/src/rust
cargo build --release --bin piper-plus-cli
```

C#/C++等の他ランタイムはビルド不要(このアプリでは未使用)。

## ビルド後の確認

各`--help`または`--version`相当のコマンドで起動できることを確認してから、`system/config.json`のポート設定に合わせてアプリから起動されることを確認する。
