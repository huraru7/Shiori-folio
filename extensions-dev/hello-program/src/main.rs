//! 詩織Ver6.0の段階1の、試しの別のプログラムの拡張機能。
//!
//! 標準入出力で、1行1つのJSONをやり取りする(詩織の決まり)。
//! - 起動したら、詩織に`log.write`を頼む(拡張機能から詩織への頼みごとの確かめ)
//! - 詩織から`greet`を頼まれたら、あいさつを返す。`crash`を頼まれたら、返事をせずに落ちる
//! - 標準入力が閉じたら終わる(詩織が落ちても残らないための決まり)

use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn send(msg: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{msg}");
    let _ = out.flush();
}

fn main() {
    // 標準エラーは、詩織のログに「注意」として入る
    eprintln!("hello-program: 起動した(pid {})", std::process::id());
    send(&json!({ "id": 1, "method": "log.write", "params": { "event": "起動した", "fields": { "pid": std::process::id().to_string() } } }));

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            // 自分の頼みごとへの返事。試しなので読み捨てる
            continue;
        };
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "greet" => {
                let who = params.get("who").and_then(Value::as_str).unwrap_or("ふらる");
                send(&json!({ "id": id, "result": { "message": format!("こんにちは、{who}。hello-programです") } }));
            }
            "crash" => {
                eprintln!("hello-program: 頼まれたので落ちる");
                std::process::exit(3);
            }
            other => {
                send(&json!({ "id": id, "error": { "code": "hello-program-001", "message": format!("知らない操作: {other}") } }));
            }
        }
    }
    // 標準入力が閉じた(詩織が終わった)
}
