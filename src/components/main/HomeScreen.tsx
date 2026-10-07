import { useEffect, useRef, useState } from "react";
import { api } from "../../api/tauri";
import ArticleDetail from "../library/ArticleDetail";
import { stripSourceExtension } from "../../lib/library";
import { useMainViewStore } from "../../store/useMainViewStore";
import { useShioriStore } from "../../store/useShioriStore";
import type { Handoff, LibraryFile, PendingItems } from "../../types";
import "./HomeScreen.css";

// 最近更新された記録の上限。実際は画面の高さに入る分だけ出す(スクロールさせないため)。
const RECENT_MAX = 5;
// 記録1行ぶんの高さ(px)。HomeScreen.cssの.home-screen__recent-itemと合わせる。
const RECENT_ROW_HEIGHT = 36;

interface Props {
  pending: PendingItems | null;
  onOpenPending: () => void;
}

interface OpenedArticle {
  source: string;
  sourceCategory: string;
  assetPath: string;
}

function greeting(hour: number): string {
  if (hour >= 5 && hour < 10) return "おはようございます";
  if (hour >= 10 && hour < 18) return "こんにちは";
  return "こんばんは";
}

function relativeDay(mtimeSec: number, now: Date): string {
  const day = new Date(mtimeSec * 1000);
  const startOf = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const diff = Math.round((startOf(now) - startOf(day)) / 86_400_000);
  if (diff <= 0) return "今日";
  if (diff === 1) return "昨日";
  if (diff < 30) return `${diff}日前`;
  return `${day.getMonth() + 1}/${day.getDate()}`;
}

function todayYmd(now: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

function formatTokens(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return String(n);
}

// ホーム画面(詩織Ver3.9)。詩織の玄関口として、状態・検索・前回の申し送り・最近の記録・
// 小さな状態行を、スクロールなしの1画面に収める。中身はタブを開くたびに読み直す
// (他のアプリやClaudeが書き足すため)。
function HomeScreen({ pending, onOpenPending }: Props) {
  const active = useMainViewStore((s) => s.active === "home");
  const searchInLibrary = useMainViewStore((s) => s.searchInLibrary);
  const setActiveView = useMainViewStore((s) => s.setActive);
  const serviceStatuses = useShioriStore((s) => s.serviceStatuses);

  const [query, setQuery] = useState("");
  const [recent, setRecent] = useState<LibraryFile[] | null>(null);
  const [handoff, setHandoff] = useState<Handoff | null | undefined>(undefined);
  const [claudeSessions, setClaudeSessions] = useState<number | null>(null);
  const [todayTokens, setTodayTokens] = useState<number | null>(null);
  const [freeGb, setFreeGb] = useState<number | null>(null);
  const [opened, setOpened] = useState<OpenedArticle | null>(null);
  const [recentRows, setRecentRows] = useState(RECENT_MAX);
  const recentRef = useRef<HTMLUListElement>(null);
  const now = new Date();

  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    const ifCurrent =
      <T,>(apply: (value: T) => void) =>
      (value: T) => {
        if (!cancelled) apply(value);
      };
    // 一覧・申し送りはRAGサーバーから取るため、起動直後はサービスの起動を待つ。
    const backend = useShioriStore.getState().ensureBackendServicesStarted().catch(() => undefined);
    backend
      .then(() => api.listAllKnowledge())
      .then(ifCurrent((files) => setRecent([...files].sort((a, b) => b.mtime - a.mtime).slice(0, RECENT_MAX))))
      .catch(ifCurrent(() => setRecent([])));
    backend
      .then(() => api.getLatestHandoff())
      .then(ifCurrent(setHandoff))
      .catch(ifCurrent(() => setHandoff(null)));
    api
      .listClaudeSessions()
      .then(ifCurrent((s) => setClaudeSessions(s.filter((x) => x.state !== "ended" && x.state !== "stale").length)))
      .catch(() => undefined);
    api
      .getClaudeStats("days7", null)
      .then(ifCurrent((s) => setTodayTokens(s.daily.find((d) => d.date === todayYmd(new Date()))?.actual ?? 0)))
      .catch(() => undefined);
    api
      .getPortableStorage()
      .then(ifCurrent((s) => setFreeGb(s.freeGb)))
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [active]);

  // 最近の記録は、画面の高さに入る行数だけ出す。
  useEffect(() => {
    const el = recentRef.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => {
      const rows = Math.floor(entry.contentRect.height / RECENT_ROW_HEIGHT);
      setRecentRows(Math.max(1, Math.min(RECENT_MAX, rows)));
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [opened]);

  if (opened) {
    return (
      <div className="home-screen home-screen--detail">
        <ArticleDetail
          source={opened.source}
          sourceCategory={opened.sourceCategory}
          assetPath={opened.assetPath}
          onBack={() => setOpened(null)}
        />
      </div>
    );
  }

  const llm = serviceStatuses["llm"];
  const searchReady = serviceStatuses["embedding"]?.healthy && serviceStatuses["rag"]?.healthy;
  const searchStarted = serviceStatuses["embedding"] && serviceStatuses["rag"];
  const pendingTotal = pending
    ? pending.tags.filter((t) => t.status === "pending").length +
      pending.projects.filter((p) => p.status === "pending").length +
      pending.inbox.filter((i) => i.reviewStatus === null).length
    : null;

  return (
    <div className="home-screen">
      <header className="home-screen__hello">
        <h2 className="home-screen__greeting">{greeting(now.getHours())}、ふらるさん</h2>
        <div className="home-screen__services">
          <ServiceBadge label="会話" state={!llm ? "starting" : llm.healthy ? "ready" : "failed"} />
          <ServiceBadge label="検索" state={!searchStarted ? "starting" : searchReady ? "ready" : "failed"} />
        </div>
      </header>

      <form
        className="home-screen__search"
        onSubmit={(e) => {
          e.preventDefault();
          if (query.trim()) searchInLibrary(query.trim());
        }}
      >
        🔍
        <input
          type="text"
          placeholder="蔵書を検索…(Enterで図書館へ)"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </form>

      {handoff && (
        <section className="home-screen__handoff">
          <button
            type="button"
            className="home-screen__handoff-head"
            onClick={() => setOpened({ source: handoff.source, sourceCategory: handoff.sourceCategory, assetPath: "" })}
            title={handoff.title}
          >
            前回の申し送り
            <span className="home-screen__dim">
              {[handoff.project, handoff.date].filter(Boolean).join(" · ")}
            </span>
          </button>
          <p className="home-screen__handoff-body">{handoff.items.join(" / ")}</p>
        </section>
      )}

      <section className="home-screen__recent-section">
        <h3 className="home-screen__heading">最近更新された記録</h3>
        <ul className="home-screen__recent" ref={recentRef}>
          {recent === null && <li className="home-screen__dim">読み込んでいます…</li>}
          {recent?.length === 0 && <li className="home-screen__dim">記録はまだありません。</li>}
          {recent?.slice(0, recentRows).map((file) => (
            <li key={file.relativePath}>
              <button
                type="button"
                className="home-screen__recent-item"
                onClick={() =>
                  setOpened({ source: file.source, sourceCategory: file.sourceCategory, assetPath: file.assetPath })
                }
                title={file.title || stripSourceExtension(file.source)}
              >
                <span className="home-screen__recent-title">{file.title || stripSourceExtension(file.source)}</span>
                <span className="home-screen__dim">{relativeDay(file.mtime, now)}</span>
              </button>
            </li>
          ))}
        </ul>
      </section>

      <footer className="home-screen__status">
        <button type="button" className="home-screen__status-item" onClick={onOpenPending}>
          要確認 {pendingTotal ?? "…"}件
        </button>
        <button type="button" className="home-screen__status-item" onClick={() => setActiveView("claude")}>
          Claude {claudeSessions ?? "…"}セッション稼働中
        </button>
        <span className="home-screen__status-item">
          今日 {todayTokens === null ? "…" : formatTokens(todayTokens)}トークン
        </span>
        <span className="home-screen__status-item">SSD 残り {freeGb === null ? "…" : `${Math.round(freeGb)}GB`}</span>
      </footer>
    </div>
  );
}

function ServiceBadge({ label, state }: { label: string; state: "ready" | "starting" | "failed" }) {
  const text = { ready: "使えます", starting: "準備中", failed: "起動できていません" }[state];
  return (
    <span className={`home-screen__service home-screen__service--${state}`} title={`${label}: ${text}`}>
      <span className="home-screen__service-dot" aria-hidden="true" />
      {label}
    </span>
  );
}

export default HomeScreen;
