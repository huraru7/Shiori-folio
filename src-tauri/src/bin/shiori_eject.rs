//! 実機SSDを安全に取り外せる状態にするCLI(詩織の取り外し機能)。
//!
//! 使用法: `shiori-eject`            … 確認だけ(何も止めない)。止める予定のものと使用中のものを表示する
//!         `shiori-eject --yes`      … 止める予定のものを停止する
//!         `shiori-eject --json`     … 結果をJSONで出す(--yesと併用できる)
//!         `shiori-eject --root <パス>` … SSD(portable/)のルートを指定する(省略時は実行ファイルの場所から探す)
//!
//! 主にClaudeがBashから呼ぶ想定。オプションなしを確認だけにしてあるのは、誤って止める事故を
//! 防ぐため。`mcp_server`も止める(止めると、使っているClaudeのセッションのMCPは使えなくなる)。
//! 取り外し(アンマウント)自体は行わない。終了コード: 0=正常(使用中が残るだけでも0)、
//! 1=止められなかったプロセスがある、2=引数の誤り。

use serde::Serialize;
use std::path::PathBuf;

use shiori_folio_lib::eject::{self, Entry, Plan, StopOutcome};
use shiori_folio_lib::project_root;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    root: String,
    executed: bool,
    plan: Plan,
    outcome: Option<StopOutcome>,
    /// `lsof`で見つかった、ほかにSSDを開いているプロセス(pid, 名前)。止めはしない。
    other_holders: Vec<(u32, String)>,
}

fn main() {
    let mut yes = false;
    let mut json = false;
    let mut root: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--yes" => yes = true,
            "--json" => json = true,
            "--root" => match args.next() {
                Some(p) => root = Some(PathBuf::from(p)),
                None => usage_exit("--rootにはパスが必要です"),
            },
            "-h" | "--help" => {
                println!("{}", USAGE);
                return;
            }
            other => usage_exit(&format!("不明な引数: {other}")),
        }
    }
    let root = root.unwrap_or_else(project_root);

    let (plan, _sys) = eject::current_plan(&root);
    let outcome = if yes && !plan.to_stop.is_empty() {
        let pids: Vec<u32> = plan.to_stop.iter().map(|e| e.pid).collect();
        Some(eject::stop_processes(&pids))
    } else {
        None
    };

    // 停止の後に、ほかにSSDを開いているものを探す(止める予定・使用中として挙げたものは除く)。
    let skip: Vec<u32> = plan.to_stop.iter().map(|e| e.pid).collect();
    let other_holders = eject::other_holders(&root, &skip);

    let report = Report {
        root: root.display().to_string(),
        executed: yes,
        plan,
        outcome,
        other_holders,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    } else {
        print_human(&report);
    }
    let failed = report.outcome.as_ref().is_some_and(|o| !o.failed.is_empty());
    std::process::exit(if failed { 1 } else { 0 });
}

const USAGE: &str = "使い方: shiori-eject [--yes] [--json] [--root <portable/のパス>]\n  オプションなし: 確認だけ(何も止めない)\n  --yes: 止める予定のものを停止する";

fn usage_exit(msg: &str) -> ! {
    eprintln!("{msg}\n{USAGE}");
    std::process::exit(2);
}

fn list(entries: &[Entry]) {
    for e in entries {
        println!("  - {} (pid {})", e.name, e.pid);
        if !e.note.is_empty() {
            println!("      {}", e.note);
        }
    }
}

fn print_human(r: &Report) {
    println!("対象のSSD: {}", r.root);
    if r.plan.to_stop.is_empty() {
        println!("止めるものはありません。");
    } else if let Some(o) = &r.outcome {
        println!("\n停止の結果:");
        let name_of = |pid: u32| {
            r.plan
                .to_stop
                .iter()
                .find(|e| e.pid == pid)
                .map(|e| e.name.clone())
                .unwrap_or_default()
        };
        for pid in &o.stopped {
            println!("  - {} (pid {pid}): 停止しました", name_of(*pid));
        }
        for pid in &o.forced {
            println!("  - {} (pid {pid}): 穏やかに止まらなかったため強制終了しました", name_of(*pid));
        }
        for pid in &o.failed {
            println!("  - {} (pid {pid}): 止められませんでした", name_of(*pid));
        }
    } else {
        println!("\n止める予定のもの(まだ止めていません。止めるには --yes):");
        list(&r.plan.to_stop);
    }

    if !r.other_holders.is_empty() {
        println!("\nほかにSSDのファイルを開いているプロセス(止めません。必要なら閉じてください):");
        for (pid, name) in &r.other_holders {
            println!("  - {name} (pid {pid})");
        }
    }

    println!();
    let failed = r.outcome.as_ref().is_some_and(|o| !o.failed.is_empty());
    if failed {
        println!("止められなかったプロセスがあります。上の一覧を確認してください。");
    } else if !r.other_holders.is_empty() {
        println!("まだSSDを使っているものがあります。取り外す前に、上の「ほかにSSDを開いているプロセス」を閉じてください。そのあと、Finder(Windowsは「ハードウェアを安全に取り外す」)で取り出してください。");
    } else if r.executed || r.plan.to_stop.is_empty() {
        println!("SSDを使っているものはありません。Finder(Windowsは「ハードウェアを安全に取り外す」)で取り出せます。");
    }
}
