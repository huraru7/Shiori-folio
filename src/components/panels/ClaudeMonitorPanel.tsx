import { useEffect, useState } from "react";
import { api } from "../../api/tauri";
import { useWindowStore } from "../../store/useWindowStore";
import type { ClaudeSession, ClaudeSessionState } from "../../types";
import "./ClaudeMonitorPanel.css";

// 外付けSSD(exFAT)上のディレクトリを読むため、読みに行く間隔は短くしすぎない。
const POLL_INTERVAL_MS = 3000;

const STATE_LABEL: Record<ClaudeSessionState, string> = {
  working: "作業中",
  waiting: "入力待ち",
  idle: "待機",
  ended: "終了",
  stale: "応答なし",
};

function formatAge(updatedAt: number, now: number): string {
  const sec = Math.max(0, Math.floor((now - updatedAt) / 1000));
  if (sec < 10) return "たった今";
  if (sec < 60) return `${sec}秒前`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}分前`;
  return `${Math.floor(min / 60)}時間前`;
}

// Claudeモニターウィンドウ(詩織Ver3.7)。Claude Code側のフックが
// data/claude-status/に書いたセッションの状況を読んで一覧表示する(読み取り専用)。
// ウィンドウが見えている間だけ読みに行く。
function ClaudeMonitorPanel() {
  const visible = useWindowStore((s) => s.windows["claude-monitor"]?.visible ?? false);
  const [sessions, setSessions] = useState<ClaudeSession[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!visible) return;
    let cancelled = false;
    const load = () => {
      api
        .listClaudeSessions()
        .then((r) => {
          if (cancelled) return;
          setSessions(r);
          setError(null);
          setNow(Date.now());
        })
        .catch((e) => {
          if (!cancelled) setError(String(e));
        });
    };
    load();
    const timer = setInterval(load, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [visible]);

  return (
    <div className="claude-monitor">
      {error && <p className="claude-monitor__error">読み取りに失敗しました: {error}</p>}
      {!error && sessions.length === 0 && (
        <p className="claude-monitor__empty">動いているClaudeのセッションはありません。</p>
      )}
      <ul className="claude-monitor__list">
        {sessions.map((s) => (
          <li key={s.sessionId} className="claude-monitor__item">
            <span className={`claude-monitor__badge claude-monitor__badge--${s.state}`}>
              {STATE_LABEL[s.state]}
            </span>
            <div className="claude-monitor__body">
              <div className="claude-monitor__project">{s.project || "(不明)"}</div>
              {s.title && <div className="claude-monitor__title">{s.title}</div>}
            </div>
            <div className="claude-monitor__meta">
              <span>{s.host}</span>
              <span>{formatAge(s.updatedAt, now)}</span>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

export default ClaudeMonitorPanel;
