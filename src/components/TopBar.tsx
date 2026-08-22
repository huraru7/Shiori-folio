import { useEffect, useState } from "react";
import { api } from "../api/tauri";
import shioriMark from "../assets/logo/shiori-mark-header.png";
import "./TopBar.css";

const WEEKDAY_LABELS = ["日", "月", "火", "水", "木", "金", "土"];

interface DateDisplayConfig {
  showYear: boolean;
  showMonth: boolean;
  showDay: boolean;
  showWeekday: boolean;
}

const DEFAULT_DATE_CONFIG: DateDisplayConfig = {
  showYear: true,
  showMonth: true,
  showDay: true,
  showWeekday: false,
};

function formatDate(now: Date, cfg: DateDisplayConfig): string {
  const parts: string[] = [];
  if (cfg.showYear) parts.push(String(now.getFullYear()));
  if (cfg.showMonth) parts.push(String(now.getMonth() + 1));
  if (cfg.showDay) parts.push(String(now.getDate()));
  const numeric = parts.join("/");
  if (!cfg.showWeekday) return numeric;
  const weekday = `(${WEEKDAY_LABELS[now.getDay()]})`;
  return numeric ? `${numeric}${weekday}` : weekday;
}

interface Props {
  leftCollapsed: boolean;
  onToggleLeftCollapsed: () => void;
}

// ヘッダー(2026-08-12、UI/UX改善指示書2章、2026-08-13ロゴ実装で仮置きから差し替え)。
// 挨拶文・今日の手入れ件数の表示は廃止し、ロゴマークに置き換えた。図書館・設定への
// 導線もヘッダーから削除し、タスクバー(Dock)側に統合している(App.tsx参照)。
// 日付の年/月/日/曜日の表示・非表示は設定画面(詳細設定)から切り替えられる。
// 左ゾーン(会話UI)の折りたたみトグル(詩織Ver3.0、UI改善4-3節)もここに配置する。
function TopBar({ leftCollapsed, onToggleLeftCollapsed }: Props) {
  const [now, setNow] = useState(new Date());
  const [dateConfig, setDateConfig] = useState<DateDisplayConfig>(DEFAULT_DATE_CONFIG);

  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 30_000);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    api
      .getConfig()
      .then((config) =>
        setDateConfig({
          showYear: config.showYear,
          showMonth: config.showMonth,
          showDay: config.showDay,
          showWeekday: config.showWeekday,
        }),
      )
      .catch(() => {
        // 取得に失敗してもヘッダー自体は表示したいので、デフォルト値のまま続行する
      });
  }, []);

  const timeText = now.toLocaleTimeString("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
  });

  return (
    <header className="top-bar">
      <div className="top-bar__logo">
        <img src={shioriMark} className="top-bar__logo-mark" alt="詩織" />
        <button
          className="top-bar__collapse-btn"
          onClick={onToggleLeftCollapsed}
          title={leftCollapsed ? "会話エリアを表示" : "会話エリアを折りたたむ"}
          aria-label={leftCollapsed ? "会話エリアを表示" : "会話エリアを折りたたむ"}
        >
          {leftCollapsed ? "▶" : "◀"}
        </button>
      </div>
      <div className="top-bar__datetime">
        <div className="top-bar__date">{formatDate(now, dateConfig)}</div>
        <div className="top-bar__time">{timeText}</div>
      </div>
    </header>
  );
}

export default TopBar;
