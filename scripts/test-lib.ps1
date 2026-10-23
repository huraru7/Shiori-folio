# 本体のライブラリのテスト(cargo test --lib)を、Windowsで動かす(詩織Ver6.0の段階1)。
#
# Windowsでは、テストのプログラムにCommon Controls v6を求めるマニフェストが付かず、
# 起動時にSTATUS_ENTRYPOINT_NOT_FOUND(0xC0000139)で止まる。Tauriの部品がv6にしか無い関数を
# 使うため。cargoの仕組み(build.rsのrustc-link-arg-tests)はライブラリの単体テストには効かないので、
# ビルドしたテストのプログラムに、Windows SDKのmt.exeでマニフェストを埋め込んでから動かす。
#
# 使い方(リポジトリの直下で): powershell -File scripts\test-lib.ps1 [テスト名の絞り込み…]
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Filter)

# cargoは進み具合を標準エラーに出すので、"Stop"にすると止まってしまう。終了コードで確かめる
$ErrorActionPreference = "Continue"
$tauriDir = Join-Path $PSScriptRoot "..\src-tauri"
$manifest = Join-Path $tauriDir "test-manifest.xml"

$mt = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\mt.exe" -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $mt) {
    Write-Host "mt.exe(Windows SDK)が見つかりません。Visual StudioのWindows SDKを入れてください。"
    exit 1
}

Push-Location $tauriDir
try {
    # テストのプログラムの場所は、cargoのJSONの出力から取る(ファイル名にハッシュが付くため)
    $lines = cargo test --release --lib --no-run --message-format=json 2>$null
    if ($LASTEXITCODE -ne 0) {
        cargo test --release --lib --no-run
        exit $LASTEXITCODE
    }
    $exe = $lines | ForEach-Object { try { $_ | ConvertFrom-Json } catch { $null } } |
        Where-Object { $_.reason -eq "compiler-artifact" -and $_.profile.test -and $_.target.name -eq "shiori_folio_lib" -and $_.executable } |
        Select-Object -Last 1 -ExpandProperty executable
    if (-not $exe) {
        Write-Host "テストのプログラムが見つかりません。"
        exit 1
    }
    & $mt.FullName -nologo -manifest $manifest "-outputresource:$exe;#1"
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & $exe @Filter
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
