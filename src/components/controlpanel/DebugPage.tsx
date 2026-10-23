import { useEffect, useState } from "react";
import { api } from "../../api/tauri";
import type { PassiveRecallStats, RagasHistory, TtsFailure } from "../../types";

// デバッグのページ(2026-08-12、UI/UX改善指示書8章で独立した全画面に分けたが、詩織Ver6.0の段階2で
// ふらるの指示により、設定の画面のほかのページと同じナビタブの1つに戻した)。読み取り専用。
export function DebugPage() {
  const [ttsFailures, setTtsFailures] = useState<TtsFailure[]>([]);
  const [passiveRecallStats, setPassiveRecallStats] = useState<PassiveRecallStats | null>(null);
  const [ragasHistory, setRagasHistory] = useState<RagasHistory | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.getTtsFailures(), api.getPassiveRecallStats(), api.getRagasHistory()])
      .then(([failures, stats, history]) => {
        setTtsFailures(failures);
        setPassiveRecallStats(stats);
        setRagasHistory(history);
      })
      .catch((err) => setError(String(err)));
  }, []);

  const hitRatePercent =
    passiveRecallStats && passiveRecallStats.total > 0
      ? Math.round((passiveRecallStats.hits / passiveRecallStats.total) * 100)
      : null;

  return (
    <>
      <div className="control-panel__page-head">
        <h1>デバッグ</h1>
        <p>読み取り専用です。開発中に確認してきた内部状態をまとめて見られます。</p>
      </div>

      {error && <div className="control-panel__error">取得に失敗しました: {error}</div>}

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">常時バックグラウンド検索(passive recall)のヒット率</div>
        {passiveRecallStats === null ? (
          <div className="control-panel__gauge-sub">取得中...</div>
        ) : passiveRecallStats.total === 0 ? (
          <div className="control-panel__gauge-sub">
            このセッションではまだ実行されていません(アプリ再起動で集計はリセットされます)
          </div>
        ) : (
          <div className="control-panel__gauge-sub">
            直近{passiveRecallStats.total}回中{passiveRecallStats.hits}回ヒット({hitRatePercent}%)
          </div>
        )}
      </div>

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">TTS失敗ログ(直近20件)</div>
        {ttsFailures.length === 0 ? (
          <div className="control-panel__gauge-sub">失敗の記録はありません</div>
        ) : (
          <div className="control-panel__service-list">
            {ttsFailures.map((f, i) => (
              <div className="control-panel__service-item" key={i}>
                <span>{f.error}</span>
                <span className="control-panel__service-status">{f.createdAt}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">RAGAS評価履歴(services/rag/eval/results/history.csv)</div>
        {ragasHistory === null ? (
          <div className="control-panel__gauge-sub">取得中...</div>
        ) : ragasHistory.rows.length === 0 ? (
          <div className="control-panel__gauge-sub">まだRAGAS評価が実行されていません</div>
        ) : (
          <div className="control-panel__debug-table-wrap">
            <table className="control-panel__debug-table">
              <thead>
                <tr>
                  {ragasHistory.headers.map((h) => (
                    <th key={h}>{h}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {ragasHistory.rows.map((row, i) => (
                  <tr key={i}>
                    {row.map((cell, j) => (
                      <td key={j}>{cell}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </>
  );
}
