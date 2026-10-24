import { useCallback, useEffect, useState } from "react";
import { api } from "../../api/tauri";
import type { DiagnoseReport } from "../../types";

// 診断のページ(詩織Ver6.0の段階6)。中身は窓を使わないCLIのshiori-doctorと同じ文章で、
// 外部のAIに貼り付けて渡すためのもの。読み取り専用(何も書き換えない)。
export function DiagnosePage() {
  const [report, setReport] = useState<DiagnoseReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState<"ok" | "ng" | null>(null);

  const load = useCallback(() => {
    setLoading(true);
    setCopied(null);
    api
      .diagnoseReport()
      .then((r) => {
        setReport(r);
        setError(null);
      })
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(load, [load]);

  const copy = () => {
    if (!report) return;
    api
      .diagnoseCopy(report.text)
      .then(() => setCopied("ok"))
      .catch(() => setCopied("ng"));
  };

  return (
    <>
      <div className="control-panel__page-head">
        <h1>診断</h1>
        <p>
          詩織の状態をまとめた文章です。うまく動かないときは「まとめてコピー」を押して、外部のAIに貼り付けてください。
          記事の本文と会話の中身は入っていません。画面が出ないときは、SSDの「診断.command」(Windowsは「診断.bat」)で同じ文章が取れます。
        </p>
      </div>

      {error && <div className="control-panel__error">診断を作れませんでした: {error}</div>}
      {report && report.ng > 0 && (
        <div className="control-panel__error" data-diagnose-ng={report.ng}>
          NGが{report.ng}件あります。下の文章の [NG] の行を見てください。
        </div>
      )}

      <div className="control-panel__panel">
        <div className="diagnose__actions">
          <button className="control-panel__btn control-panel__btn--primary" onClick={copy} disabled={!report || loading}>
            まとめてコピー
          </button>
          <button className="control-panel__btn" onClick={load} disabled={loading}>
            {loading ? "作っています…" : "作り直す"}
          </button>
          {copied === "ok" && <span className="control-panel__gauge-sub">コピーしました</span>}
          {copied === "ng" && <span className="control-panel__gauge-sub">コピーできませんでした</span>}
        </div>
        <pre className="diagnose__text" data-diagnose-text>
          {report ? report.text : loading ? "作っています…" : ""}
        </pre>
      </div>
    </>
  );
}
