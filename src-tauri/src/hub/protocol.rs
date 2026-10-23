//! 画面の部品の拡張機能を配る、専用のURLの仕組み(詩織Ver6.0の段階1)。
//!
//! `shiori-ext://localhost/{名前}/{ファイル}`(Windowsでは`http://shiori-ext.localhost/{名前}/{ファイル}`)で、
//! 承認済みの画面の部品の拡張機能のフォルダの中のファイルだけを返す。`/_shiori/shiori.js`は、
//! 拡張機能が詩織に頼みごとをするための小さな部品(どの拡張機能からも読める)。
//! `/_shiori/icons/{id}.png`は、左の縦の列の本体の画面のアイコンの差し替え(段階2、
//! `portable/data/ui/icons/`に置いたPNG)。
//!
//! 返すときに守りの指定(CSP)を付け、ネットへの接続と、ほかの場所からの読み込みをできなくする。
//! 本体の画面は、これを`<iframe sandbox="allow-scripts">`で開く(`allow-same-origin`を付けないので、
//! 拡張機能は本体の画面やTauriの窓口に触れない)。

use super::manifest::{self, Kind};
use super::HubState;
use crate::error_codes;
use crate::shiori_log;
use std::borrow::Cow;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext};

pub const SCHEME: &str = "shiori-ext";

/// 拡張機能のページに付ける守りの指定。
/// `'self'`は、囲い(sandbox)の中でも拡張機能自身のURLを指す(WebView2とWKWebViewで確かめる。企画書の未確定事項)。
pub const EXTENSION_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
     img-src 'self' data:; font-src 'self'; connect-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'";

/// 拡張機能が詩織に頼みごとをするための部品(`shiori.call(method, params)`)。
const BRIDGE_JS: &str = include_str!("shiori-bridge.js");

pub fn handle<R: Runtime>(ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = percent_decode(request.uri().path());
    let path = path.trim_start_matches('/');
    if path == "_shiori/shiori.js" {
        return ok(BRIDGE_JS.as_bytes().to_vec(), "text/javascript; charset=utf-8");
    }
    if let Some(file) = path.strip_prefix("_shiori/icons/") {
        return match crate::sidebar_layout::icon_override(file) {
            Some(bytes) => ok(bytes, "image/png"),
            None => not_found(path, "差し替えのアイコンが無い"),
        };
    }
    let Some((name, file)) = path.split_once('/') else {
        return not_found(path, "名前とファイルの形になっていない");
    };
    let file = if file.is_empty() { "index.html" } else { file };
    if !manifest::is_inside_relative(file) {
        return not_found(path, "フォルダの外を指している");
    }
    let state = ctx.app_handle().state::<HubState>();
    let Ok(hub) = state.0.lock() else {
        return not_found(path, "ハブの状態を読めない");
    };
    let Some(ext) = hub.ready(name) else {
        return not_found(path, "承認済みの拡張機能ではない");
    };
    if ext.manifest.as_ref().map(|m| m.kind) != Some(Kind::Ui) {
        return not_found(path, "画面の部品の拡張機能ではない");
    }
    let full = ext.dir.join(file);
    drop(hub);
    match std::fs::read(&full) {
        Ok(bytes) => {
            shiori_log::debug("HUB", "拡張機能のファイルを返した", &[("name", &name), ("ファイル", &file)]);
            ok(bytes, mime_of(file))
        }
        Err(_) => not_found(path, "ファイルが無い"),
    }
}

fn ok(body: Vec<u8>, mime: &str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header("Content-Security-Policy", EXTENSION_CSP)
        .header("X-Content-Type-Options", "nosniff")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Cow::Owned(body))
        .unwrap()
}

fn not_found(path: &str, why: &str) -> Response<Cow<'static, [u8]>> {
    shiori_log::warn(
        error_codes::EXT_013.code,
        "拡張機能のファイルを返さなかった",
        &[("道", &path), ("理由", &why)],
    );
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Cow::Owned(format!("{}: {why}", error_codes::EXT_013.code).into_bytes()))
        .unwrap()
}

pub fn mime_of(file: &str) -> &'static str {
    let ext = file.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

/// URLの`%XX`を元の文字に戻す(日本語のファイル名のため)。壊れた並びは、そのまま残す。
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(b) = hex {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn パーセントの戻し() {
        assert_eq!(percent_decode("/hello-ui/index.html"), "/hello-ui/index.html");
        assert_eq!(percent_decode("/a/%E7%94%BB%E5%83%8F.png"), "/a/画像.png");
        assert_eq!(percent_decode("/a/%2e%2e/x"), "/a/../x");
        assert_eq!(percent_decode("/bad%zz"), "/bad%zz");
        assert_eq!(percent_decode("/end%4"), "/end%4");
    }

    #[test]
    fn ファイルの種類() {
        assert_eq!(mime_of("index.html"), "text/html; charset=utf-8");
        assert_eq!(mime_of("a/app.JS"), "text/javascript; charset=utf-8");
        assert_eq!(mime_of("noext"), "application/octet-stream");
    }
}
