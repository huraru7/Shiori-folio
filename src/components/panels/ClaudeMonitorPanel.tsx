import { useState } from "react";
import { useWindowStore } from "../../store/useWindowStore";
import ClaudeSessionsView from "./ClaudeSessionsView";
import ClaudeStatsView from "./ClaudeStatsView";
import "./ClaudeMonitorPanel.css";

type Tab = "sessions" | "stats";

// Claudeモニターウィンドウ。稼働状況(詩織Ver3.7)と利用統計(詩織Ver3.9)をタブで切り替える。
function ClaudeMonitorPanel() {
  const visible = useWindowStore((s) => s.windows["claude-monitor"]?.visible ?? false);
  const [tab, setTab] = useState<Tab>("sessions");

  return (
    <div className="claude-monitor">
      <div className="claude-monitor__tabs" role="tablist">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "sessions"}
          className={`claude-monitor__tab${tab === "sessions" ? " claude-monitor__tab--active" : ""}`}
          onClick={() => setTab("sessions")}
        >
          稼働状況
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "stats"}
          className={`claude-monitor__tab${tab === "stats" ? " claude-monitor__tab--active" : ""}`}
          onClick={() => setTab("stats")}
        >
          利用統計
        </button>
      </div>
      {tab === "sessions" ? (
        <ClaudeSessionsView active={visible} />
      ) : (
        <ClaudeStatsView active={visible} />
      )}
    </div>
  );
}

export default ClaudeMonitorPanel;
