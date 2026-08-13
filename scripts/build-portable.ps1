# ============================================================================
# 【未検証】このスクリプトはWindows実機で一度も実行・確認されていません。
# Mac版(build-portable.sh)の設計・教訓を踏まえて書いたドラフトです。
# 実行前に必ず内容を読み、小さな単位(まずエンジンバイナリのコピーだけ、等)
# で試しながら検証してください。特に以下は机上の想定であり実機確認が必須です:
#   - llama-server.exe(CUDA版)が依存するDLL一式(cudart64_*.dll等)の
#     実際の配置場所と、コピーだけで自己完結するか(CUDA Toolkitの
#     ライセンス上、どのDLLを同梱してよいかも要確認)
#   - dumpbin/depends的な依存解析ツールがこの開発機にあるか
#     (Visual Studio Build Toolsが必要な可能性がある)
#   - uvのWindows版でのPythonインストール・パッケージインストール手順
#   - ランチャー(.bat)の相対パス解決とダブルクリック起動の実際の挙動
# ============================================================================
#
# 段階G: portable/ 運用パッケージ生成スクリプト(Windows)。
# 配置場所: system/scripts/build-portable.ps1
#
# Mac版(build-portable.sh)からの教訓:
#   - 実行ファイルは@rpath的な仕組み(Windowsでは既定でexeと同じフォルダを
#     DLL探索するため、Macほど深刻ではない見込みだが、CUDA/OpenSSL等
#     Windows側のツールチェーン外にあるDLLは別途コピーが必要になりうる)
#   - venvは移動に弱い(Mac実機ではbin/pythonが移動元への絶対パス
#     シンボリックリンクになり、USB上の別ドライブへ移動すると壊れることを
#     確認した)。venvを使わず、uvで取得した可搬版Python本体に直接
#     パッケージをインストールする方式にしている(移動後も動作することを
#     Mac実機で確認済み。Windows側も同じuvコマンド体系のはずだが要検証)
#   - 絶対パスを一切焼き込まない(このスクリプト自身の場所を基準に解決する)
#
# Shiori-folio/直下(system/・library/と並ぶ場所、このスクリプトから見て
# 2階層上)にportable/を生成する。Mac側と同じportable/(例えばUSB上)を
# 指して実行すれば、bin/win/が追加される形でマージされる。

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SystemDir = (Resolve-Path (Join-Path $ScriptDir "..")).Path
$RepoRoot = (Resolve-Path (Join-Path $SystemDir "..")).Path
$LibraryDir = Join-Path $RepoRoot "library"
$PortableDir = Join-Path $RepoRoot "portable"

Write-Host "=== portable/ 生成開始 (OS: win) ==="
Write-Host "出力先: $PortableDir"

if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
    Write-Error "uv が見つかりません(RAG用ポータブルPython環境の構築に使用します)。https://astral.sh/uv からインストールしてください。"
    exit 1
}

New-Item -ItemType Directory -Force -Path $PortableDir | Out-Null

# --- 1. 共有リソース(OSを問わず同じ内容。実行のたびに作り直す) ---
# Mac版のrsync --deleteに相当する処理をRobocopyで行う(/MIR = ミラーリング)。

Write-Host "--- prompts/ ---"
robocopy "$SystemDir\prompts" "$PortableDir\prompts" /MIR /NFL /NDL /NJH /NJS | Out-Null

Write-Host "--- config.json ---"
Copy-Item "$SystemDir\config.json" "$PortableDir\config.json" -Force

Write-Host "--- models/ ---"
robocopy "$SystemDir\models" "$PortableDir\models" /E /NFL /NDL /NJH /NJS | Out-Null

Write-Host "--- data/vectordb, data/shiori.db ---"
New-Item -ItemType Directory -Force -Path "$PortableDir\data" | Out-Null
robocopy "$SystemDir\data\vectordb" "$PortableDir\data\vectordb" /MIR /NFL /NDL /NJH /NJS | Out-Null
Copy-Item "$SystemDir\data\shiori.db" "$PortableDir\data\shiori.db" -Force

Write-Host "--- library/ (system/の外、実データをコピー) ---"
robocopy "$LibraryDir" "$PortableDir\library" /MIR /NFL /NDL /NJH /NJS | Out-Null

Write-Host "--- services/rag/ (ソースのみ。.venv/__pycache__/evalは除外) ---"
robocopy "$SystemDir\services\rag" "$PortableDir\services\rag" /MIR /NFL /NDL /NJH /NJS `
    /XD ".venv" "__pycache__" "eval" | Out-Null

# --- 2. OS別バイナリ配置(bin/win/) ---

$BinDir = Join-Path $PortableDir "bin\win"
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

# shiori-folio本体。tauri buildの生成物のうち、インストーラ(nsis/msi)ではなく
# target/release/shiori-folio.exeそのもの(単体で動く実行ファイル)を使う
# (Mac版で.app全体をそのままコピーしたのと同じ考え方)。
$ExeSrc = Join-Path $SystemDir "src-tauri\target\release\shiori-folio.exe"
if (-not (Test-Path $ExeSrc)) {
    Write-Error "$ExeSrc が見つかりません。先に (cd system && npx tauri build) を実行してください。"
    exit 1
}
Write-Host "--- shiori-folio.exe ---"
Copy-Item $ExeSrc (Join-Path $BinDir "shiori-folio.exe") -Force

# --- エンジンバイナリ ---
# 【未検証】WindowsのCMakeビルドはDLLをexeと同じ出力フォルダ(bin/Release/)に
# 自動配置することが多く、Macのdylibbundlerほど大掛かりな作業は不要な見込み
# だが、CUDA Toolkit本体のDLL(cudart64_*.dll、cublas64_*.dll等)は
# 別途CUDA_PATHから探してコピーする必要がある可能性が高い。以下は
# 「ビルド出力フォルダの中身をまるごとコピーする」という単純な実装に
# とどめてあるので、実機で不足DLLが無いか(起動時のエラーダイアログや
# `where`コマンドでの確認)を確かめること。
Write-Host "--- llama-server (DLL一式込み) ---"
$LlamaBinDir = Join-Path $SystemDir "third_party\llama.cpp\build\bin\Release"
if (-not (Test-Path $LlamaBinDir)) {
    $LlamaBinDir = Join-Path $SystemDir "third_party\llama.cpp\build\bin"
}
New-Item -ItemType Directory -Force -Path (Join-Path $BinDir "libs-llama") | Out-Null
Copy-Item (Join-Path $LlamaBinDir "llama-server.exe") (Join-Path $BinDir "llama-server.exe") -Force
Get-ChildItem $LlamaBinDir -Filter "*.dll" | Copy-Item -Destination (Join-Path $BinDir "libs-llama") -Force

Write-Host "--- whisper-server (DLL一式込み) ---"
$WhisperBinDir = Join-Path $SystemDir "third_party\whisper.cpp\build\bin\Release"
if (-not (Test-Path $WhisperBinDir)) {
    $WhisperBinDir = Join-Path $SystemDir "third_party\whisper.cpp\build\bin"
}
New-Item -ItemType Directory -Force -Path (Join-Path $BinDir "libs-whisper") | Out-Null
Copy-Item (Join-Path $WhisperBinDir "whisper-server.exe") (Join-Path $BinDir "whisper-server.exe") -Force
Get-ChildItem $WhisperBinDir -Filter "*.dll" | Copy-Item -Destination (Join-Path $BinDir "libs-whisper") -Force

Write-Host "--- piper-plus-cli ---"
$PiperExe = Join-Path $SystemDir "third_party\piper-plus\src\rust\target\release\piper-plus-cli.exe"
Copy-Item $PiperExe (Join-Path $BinDir "piper-plus-cli.exe") -Force
# 【未検証】piper-plus-cliがONNX RuntimeのDLL(onnxruntime.dll等)を動的に
# 要求する場合、同じフォルダに無いと起動できない。Mac版はCoreML経由の
# 静的リンクで自己完結していたが、Windows(CUDA Execution Provider)側は
# 別途DLLコピーが必要になる可能性が高いので確認すること。

# NOTE: 現状のRust側resolve_engine_exe/piper_clientは、llama-server/
# whisper-server本体をbin/<os>/直下に、依存DLLをlibs-llama/・libs-whisper/
# に置く構成を前提にしている。ただしWindowsのDLL探索は既定で「exeと同じ
# フォルダ」を見るため、libs-llama/・libs-whisper/配下のDLLは自動では
# 見つからない可能性がある。動作しない場合は、DLLをexeと同じbin/win/直下に
# 展開する(libsサブフォルダを使わない)か、Rust側でSetDllDirectory相当の
# 対応を追加することを検討すること。

# --- 3. RAG用の可搬版Python環境(bin/win/rag-venv/) ---
#
# Mac版の教訓: uv venvはbin/pythonが移動元への絶対パスシンボリックリンクに
# なり、USBの別ドライブ文字へ移動すると壊れることを確認した。そのため
# venvを介さず「uv python installで取得したポータブルPython本体に
# 直接パッケージをインストールする」方式にしている。requirements.txtの
# うちragas(評価専用スクリプトeval/でのみ使用、サーバー本体は不使用)は
# 意図的に除外している。
# 【未検証】Windows版のuv python installも同様に相対移動へ耐えるはずだが
# (Windowsの実行ファイルは元々シンボリックリンクに依存しにくい)、
# 実機で必ず「別ドライブへ移動しても動くか」を確認すること。
Write-Host "--- RAG用ポータブルPython環境(bin/win/rag-venv/) ---"
$RagVenv = Join-Path $BinDir "rag-venv"
if (Test-Path $RagVenv) { Remove-Item $RagVenv -Recurse -Force }
$TmpPyDir = "$RagVenv.tmp"
New-Item -ItemType Directory -Force -Path $TmpPyDir | Out-Null
uv python install -i $TmpPyDir 3.12
$InstalledDir = Get-ChildItem $TmpPyDir -Directory -Filter "cpython-*" | Select-Object -First 1
Move-Item $InstalledDir.FullName $RagVenv
Remove-Item $TmpPyDir -Recurse -Force

$RagPython = Join-Path $RagVenv "python.exe"
uv pip install --python $RagPython --break-system-packages `
    chromadb fastapi "uvicorn[standard]" httpx tiktoken sentence-transformers

# --- 4. ランチャー ---

Write-Host "--- ランチャー(詩織を起動.bat) ---"
$LauncherPath = Join-Path $PortableDir "詩織を起動.bat"
$LauncherContent = @'
@echo off
REM 詩織(Shiori-folio) ポータブル版ランチャー(Windows)。
REM このファイル自身の場所を基準に相対パスで解決するため、USBのどの
REM ドライブ文字に挿しても動作する(絶対パスを焼き込まない。段階Cの
REM project_root()修正と同じ考え方)。
REM 【未検証】実機での起動確認が必要。
set DIR=%~dp0
start "" "%DIR%bin\win\shiori-folio.exe"
'@
Set-Content -Path $LauncherPath -Value $LauncherContent -Encoding ASCII

Write-Host "=== portable/ 生成完了 ==="
$Size = (Get-ChildItem $PortableDir -Recurse | Measure-Object -Property Length -Sum).Sum / 1GB
Write-Host ("{0:N1} GB" -f $Size)
