# ============================================================================
# 【一部検証済み】エンジンバイナリのコピー・DLL解決方式は、2026-08-13に
# third_party/ビルド出力を切り出した小規模テストで実機確認済み(下記2点)。
# uvのPythonインストール手順・ランチャー(.bat)のダブルクリック起動は
# 未検証のまま残っているため、そちらは引き続き小さな単位で確認すること。
#
# 【確認済み】DLL探索はPATH方式(子プロセスのPATH先頭にlibs-llama/・
# libs-whisper/を追加)で解決できる。exeとDLLを同一フォルダに展開する
# 必要はない(Rust側はsrc-tauri/src/lib.rsのspawn_server/extended_pathで
# 同じ方式を実装済み)。
#
# 【確認済み】llama-server.exe・whisper-server.exeはビルド出力に
# cudart64_*.dll等のCUDA Toolkit本体DLLを含んでおらず、開発機に
# グローバルインストールされたCUDA ToolkitをPATH経由で見ていることを
# 確認した。そのため、CUDA未インストールの配布先機でも動くよう、
# 以下3つを明示的にlibs-llama/・libs-whisper/それぞれに同梱する
# (cudart64_12.dll 0.5MB、cublas64_12.dll 108MB、cublasLt64_12.dll 643MB。
# 除くと`0xC0000135`で起動不能になることを実機確認済み)。
# NVIDIA CUDA Toolkit EULA上、これらランタイムDLLの再配布は一般的に
# 許容されているが、最終的な配布可否はライセンス文面を各自確認すること。
#
# 【確認済み】piper-plus-cli.exeはonnxruntime.dll・DirectML.dllとも
# 同梱不要(ONNX Runtimeは静的リンク済みで、実際にDLL無しの単体フォルダで
# 音声合成が成功することを確認した)。ただし現状のビルドはCUDA実行
# プロバイダのCargo featureが有効化されておらず、--device cudaを渡しても
# 実際はCPU推論にフォールバックしている(ポータブル化とは別件の既知事項)。
# ============================================================================
#
# 段階G: portable/ 運用パッケージ生成スクリプト(Windows)。
# 配置場所: system/scripts/build-portable.ps1
#
# Mac版(build-portable.sh)からの教訓:
#   - DLL探索・CUDA本体DLLの同梱要否は上のヘッダー注記の通り実機確認済み
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
# DLL探索はPATH方式(2026-08-13実機確認済み)。exe本体はbin/win/直下、
# 依存DLL(ビルド出力のggml-*.dll等)はlibs-llama/・libs-whisper/に分けて
# 配置する。Rust側(src-tauri/src/lib.rsのspawn_server/extended_path)が
# 起動時に子プロセスのPATH先頭へlibs-llama/・libs-whisper/を追加する実装に
# なっているため、exeと同じフォルダにDLLを展開する必要はない。

# CUDA Toolkit本体のDLL(開発機にグローバルインストールされたものをPATH経由で
# 見ていた分。配布先機にCUDA Toolkitが入っているとは限らないため明示的に同梱する。
# 実機確認済みの必要最小構成: cudart64_12.dll・cublas64_12.dll・cublasLt64_12.dll。
# cublasLt64_12.dllのみ643MBと大きいが、外すと0xC0000135で起動不能になることを
# 実機確認済みなので削れない)。CUDA_PATHが無い/バージョンが異なる環境では
# ビルド自体を継続しつつ警告に留める(このビルドスクリプト自体は非CUDA機でも
# エンジンバイナリ以外の部分を試せるようにするため)。
$CudaBinDir = $env:CUDA_PATH
if ($CudaBinDir) { $CudaBinDir = Join-Path $CudaBinDir "bin" }
if (-not $CudaBinDir -or -not (Test-Path $CudaBinDir)) {
    $CudaBinDir = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.8\bin"
}
$CudaRuntimeDlls = @("cudart64_12.dll", "cublas64_12.dll", "cublasLt64_12.dll")
function Copy-CudaRuntimeDlls([string]$DestDir) {
    if (-not (Test-Path $CudaBinDir)) {
        Write-Warning "CUDA Toolkitが見つかりません($CudaBinDir)。cudart64_12.dll等の同梱をスキップします(このexeはCUDA未同梱では起動できません)。"
        return
    }
    foreach ($dll in $CudaRuntimeDlls) {
        $src = Join-Path $CudaBinDir $dll
        if (Test-Path $src) {
            Copy-Item $src $DestDir -Force
        } else {
            Write-Warning "$dll が $CudaBinDir に見つかりません。バージョン(v12.8想定)が異なる可能性があります。"
        }
    }
}

Write-Host "--- llama-server (DLL一式込み) ---"
$LlamaBinDir = Join-Path $SystemDir "third_party\llama.cpp\build\bin\Release"
if (-not (Test-Path $LlamaBinDir)) {
    $LlamaBinDir = Join-Path $SystemDir "third_party\llama.cpp\build\bin"
}
$LlamaLibsDir = Join-Path $BinDir "libs-llama"
New-Item -ItemType Directory -Force -Path $LlamaLibsDir | Out-Null
Copy-Item (Join-Path $LlamaBinDir "llama-server.exe") (Join-Path $BinDir "llama-server.exe") -Force
Get-ChildItem $LlamaBinDir -Filter "*.dll" | Copy-Item -Destination $LlamaLibsDir -Force
Copy-CudaRuntimeDlls $LlamaLibsDir

Write-Host "--- whisper-server (DLL一式込み) ---"
$WhisperBinDir = Join-Path $SystemDir "third_party\whisper.cpp\build\bin\Release"
if (-not (Test-Path $WhisperBinDir)) {
    $WhisperBinDir = Join-Path $SystemDir "third_party\whisper.cpp\build\bin"
}
$WhisperLibsDir = Join-Path $BinDir "libs-whisper"
New-Item -ItemType Directory -Force -Path $WhisperLibsDir | Out-Null
Copy-Item (Join-Path $WhisperBinDir "whisper-server.exe") (Join-Path $BinDir "whisper-server.exe") -Force
Get-ChildItem $WhisperBinDir -Filter "*.dll" | Copy-Item -Destination $WhisperLibsDir -Force
Copy-CudaRuntimeDlls $WhisperLibsDir

Write-Host "--- piper-plus-cli ---"
# 【確認済み】ONNX Runtimeは静的リンク済みで、onnxruntime.dll/DirectML.dllとも
# 同梱不要(単体フォルダでの音声合成成功を実機確認済み)。exe本体のみコピーする。
$PiperExe = Join-Path $SystemDir "third_party\piper-plus\src\rust\target\release\piper-plus-cli.exe"
Copy-Item $PiperExe (Join-Path $BinDir "piper-plus-cli.exe") -Force

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
#
# 【確認済み・2026-08-13】`uv python install`は実体ディレクトリ
# (cpython-3.12.13-windows-x86_64-none)と、それを指すジャンクション
# (cpython-3.12-windows-x86_64-none、パッチバージョン省略のエイリアス)を
# 両方生成する。単純に"cpython-*"でフィルタしてSelect-Object -First 1すると
# アルファベット順でジャンクションの方を先に拾ってしまい、Move-Itemで
# リンクだけを移動した後にRemove-Item $TmpPyDirで参照先の実体を消してしまう
# (実機で再現・原因特定済み)。LinkTypeが無い(=ジャンクションではない)
# 実体ディレクトリだけを対象にする。
Write-Host "--- RAG用ポータブルPython環境(bin/win/rag-venv/) ---"
$RagVenv = Join-Path $BinDir "rag-venv"
if (Test-Path $RagVenv) { Remove-Item $RagVenv -Recurse -Force }
$TmpPyDir = "$RagVenv.tmp"
New-Item -ItemType Directory -Force -Path $TmpPyDir | Out-Null
uv python install -i $TmpPyDir 3.12
$InstalledDir = Get-ChildItem $TmpPyDir -Directory -Filter "cpython-*" |
    Where-Object { -not $_.LinkType } | Select-Object -First 1
if (-not $InstalledDir) {
    Write-Error "uv python installの実体ディレクトリが見つかりません($TmpPyDir 配下)。"
    exit 1
}
Move-Item $InstalledDir.FullName $RagVenv
Remove-Item $TmpPyDir -Recurse -Force

$RagPython = Join-Path $RagVenv "python.exe"
uv pip install --python $RagPython --break-system-packages `
    chromadb fastapi "uvicorn[standard]" httpx tiktoken sentence-transformers

# --- 4. ランチャー ---

Write-Host "--- ランチャー(詩織を起動.vbs / _launch.bat) ---"
# 【確認済み・2026-08-13】.batのREMコメントに日本語を書いて-Encoding ASCIIで
# 保存すると、非ASCII文字が失われて文字化けすることを実機確認済み(実行自体は
# コメントなので壊れないが、内容が読めなくなるのは事故のもと)。batファイルは
# コードページ依存を避けるため、コメントは意図的にASCII(ローマ字)のみで書く。
#
# 【2026-08-14追加】.batを直接ダブルクリックするとcmd.exeのコンソール窓が
# 一瞬表示される(startで即exeを起動して終了するだけでも、cmd自体の窓は
# 開いてから閉じるまでの間、目に見えてしまう)。これを避けるため、ユーザーが
# ダブルクリックする対象は.batではなくVBScript(.vbs)にし、
# WScript.Shell.Runのウィンドウスタイル0(非表示)経由で.batを呼び出す。
# .bat自体は内部実装として残し、_launch.batという分かりにくい名前にして
# 誤ってユーザーが直接開かないようにする。
$InternalBatPath = Join-Path $PortableDir "_launch.bat"
$InternalBatContent = @'
@echo off
REM Shiori-folio portable launcher (internal, invoked by launcher .vbs).
REM Resolves paths relative to this file's own location, so it works
REM from any drive letter (e.g. when run from a USB stick).
set DIR=%~dp0
start "" "%DIR%bin\win\shiori-folio.exe"
'@
Set-Content -Path $InternalBatPath -Value $InternalBatContent -Encoding ASCII

$VbsPath = Join-Path $PortableDir "詩織を起動.vbs"
$VbsContent = @'
Dim shell, fso, dir
Set shell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")
dir = fso.GetParentFolderName(WScript.ScriptFullName)
shell.Run """" & dir & "\_launch.bat""", 0, False
'@
Set-Content -Path $VbsPath -Value $VbsContent -Encoding ASCII

Write-Host "=== portable/ 生成完了 ==="
$Size = (Get-ChildItem $PortableDir -Recurse | Measure-Object -Property Length -Sum).Sum / 1GB
Write-Host ("{0:N1} GB" -f $Size)
