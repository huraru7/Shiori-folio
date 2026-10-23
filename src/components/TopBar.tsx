import { useEffect, useState } from "react";
import { useDateDisplayStore, type DateDisplayConfig } from "../store/useDateDisplayStore";
import { useAppModeStore } from "../store/useAppModeStore";
import { pendingBadgeCount, usePendingStore } from "../store/usePendingStore";
import shioriMark from "../assets/logo/shiori-mark-header.png";
import "./TopBar.css";

const WEEKDAY_LABELS = ["日", "月", "火", "水", "木", "金", "土"];

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

// ヘッダー(2026-08-12、UI/UX改善指示書2章、2026-08-13ロゴ実装で仮置きから差し替え)。
// 挨拶文・今日の手入れ件数の表示は廃止し、ロゴマークに置き換えた。図書館・設定への
// 導線もヘッダーから削除し、右ゾーンのタブ(MainArea)側に統合している。
// 日付の年/月/日/曜日と時刻の秒の表示・非表示は設定画面(詳細設定)から切り替えられ、
// 保存するとすぐ反映される(useDateDisplayStore)。
// 会話の場所の折りたたみは、詩織Ver6.0の段階2で会話を左の列の画面の1つにしたため無くした。
// 「要確認」は、上のタブの右から上の帯に移した(件数の印は、知らせの色の橙)。
function TopBar() {
  const [now, setNow] = useState(new Date());
  const dateConfig = useDateDisplayStore((s) => s.config);
  const mode = useAppModeStore((s) => s.mode);
  const switchingMode = useAppModeStore((s) => s.switching);
  const changeMode = useAppModeStore((s) => s.change);
  const pending = usePendingStore((s) => s.items);
  const setPendingOpen = usePendingStore((s) => s.setOpen);
  const pendingCount = pending ? pendingBadgeCount(pending) : 0;

  useEffect(() => {
    // 秒を出さないときも、分の切り替わりが遅れないよう1秒ごとに更新する。
    const timer = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    useDateDisplayStore.getState().load();
  }, []);

  const timeText = now.toLocaleTimeString("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    ...(dateConfig.showSeconds && { second: "2-digit" }),
  });

  return (
    <header className="top-bar">
      <div className="top-bar__logo">
        <img src={shioriMark} className="top-bar__logo-mark" alt="詩織" />
      </div>
      {mode && (
        <div className="top-bar__mode" role="group" aria-label="モード">
          <button
            type="button"
            aria-pressed={mode === "conversation"}
            className={`top-bar__mode-btn${mode === "conversation" ? " top-bar__mode-btn--active" : ""}`}
            disabled={switchingMode}
            onClick={() => changeMode("conversation")}
            title="詩織と会話するモード(会話用のAIを使う)"
          >
            会話
          </button>
          <button
            type="button"
            aria-pressed={mode === "external"}
            className={`top-bar__mode-btn${mode === "external" ? " top-bar__mode-btn--active" : ""}`}
            disabled={switchingMode}
            onClick={() => changeMode("external")}
            title="Claudeなど外部のAIと使うモード(会話用のAIを止めてVRAMを空ける)"
          >
            外部AI
          </button>
        </div>
      )}
      <button
        type="button"
        className="top-bar__pending"
        data-ripple
        title="要確認の記録"
        onClick={() => setPendingOpen(true)}
      >
        要確認
        {pendingCount > 0 && <span className="top-bar__badge">{pendingCount}</span>}
      </button>
      <div className="top-bar__datetime">
        <div className="top-bar__date">{formatDate(now, dateConfig)}</div>
        <div className="top-bar__time">{timeText}</div>
      </div>
    </header>
  );
}

export default TopBar;
