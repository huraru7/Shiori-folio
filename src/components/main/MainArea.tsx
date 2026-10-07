import { useEffect, useState, type ReactNode } from "react";
import { api } from "../../api/tauri";
import { useMainViewStore, type MainView } from "../../store/useMainViewStore";
import type { PendingItems } from "../../types";
import KnowledgePanel from "../panels/KnowledgePanel";
import ClaudeMonitorPanel from "../panels/ClaudeMonitorPanel";
import LibraryScreen from "../library/LibraryScreen";
import LibraryBrowseScreen from "../library/LibraryBrowseScreen";
import ControlPanel from "../controlpanel/ControlPanel";
import HomeScreen from "./HomeScreen";
import PendingModal from "./PendingModal";
import "./MainArea.css";

interface Props {
  // デバッグは内部が複雑な既存UIのため、引き続き独立した全画面表示にする。
  onOpenDebug: () => void;
}

interface ViewDef {
  id: MainView;
  label: string;
  content: ReactNode;
  // 検索バー固定+一覧スクロールのように、中身が自前でスクロールを管理する画面。
  // それ以外は余白を付けて画面ごとスクロールさせる。
  selfScroll: boolean;
}

// ホーム以外の画面。ホームは要確認の件数を受け取るため、MainArea内で組み立てる。
const VIEWS: ViewDef[] = [
  { id: "knowledge", label: "ナレッジ", content: <KnowledgePanel />, selfScroll: false },
  { id: "library", label: "図書館", content: <LibraryScreen />, selfScroll: true },
  { id: "library-browse", label: "全件閲覧", content: <LibraryBrowseScreen />, selfScroll: true },
  { id: "claude", label: "Claude", content: <ClaudeMonitorPanel />, selfScroll: false },
  { id: "settings", label: "設定", content: <ControlPanel />, selfScroll: true },
];

function pendingBadgeCount(items: PendingItems): number {
  // バッジは「未着手」のみを数える(保留中はモーダルの別タブで確認する対象で、
  // 対応不要なものとして未着手カウントから除外する、詩織Ver2.0 Phase 8の方針)。
  return (
    items.tags.filter((t) => t.status === "pending").length +
    items.projects.filter((p) => p.status === "pending").length +
    items.inbox.filter((i) => i.reviewStatus === null).length
  );
}

// 右ゾーン(詩織Ver3.9)。上のタブで画面を1つずつ切り替える。切り替えで検索結果や
// 選んだページが消えないよう、全画面を常にマウントしたまま、選ばれていない画面は
// display:noneで隠す。
function MainArea({ onOpenDebug }: Props) {
  const active = useMainViewStore((s) => s.active);
  const setActive = useMainViewStore((s) => s.setActive);
  const [pending, setPending] = useState<PendingItems | null>(null);
  const [pendingOpen, setPendingOpen] = useState(false);

  useEffect(() => {
    api
      .listPendingItems()
      .then(setPending)
      .catch(() => {
        // 取得に失敗しても、ボタン自体は使えるようにバッジを出さないだけにする。
      });
  }, []);

  const pendingCount = pending ? pendingBadgeCount(pending) : 0;
  const views: ViewDef[] = [
    {
      id: "home",
      label: "ホーム",
      content: <HomeScreen pending={pending} onOpenPending={() => setPendingOpen(true)} />,
      selfScroll: true,
    },
    ...VIEWS,
  ];

  return (
    <div className="main-area">
      <div className="main-area__bar">
        <nav className="main-area__tabs" role="tablist">
          {views.map((v) => (
            <button
              key={v.id}
              type="button"
              role="tab"
              aria-selected={active === v.id}
              className={`main-area__tab${active === v.id ? " main-area__tab--active" : ""}`}
              onClick={() => setActive(v.id)}
            >
              {v.label}
            </button>
          ))}
        </nav>
        <div className="main-area__actions">
          <button
            type="button"
            className="main-area__action"
            title="要確認の記録"
            onClick={() => setPendingOpen(true)}
          >
            要確認
            {pendingCount > 0 && <span className="main-area__badge">{pendingCount}</span>}
          </button>
          <button type="button" className="main-area__action" title="デバッグ" onClick={onOpenDebug}>
            デバッグ
          </button>
        </div>
      </div>

      {views.map((v) => (
        <div
          key={v.id}
          role="tabpanel"
          className={`main-area__pane${v.selfScroll ? " main-area__pane--self-scroll" : ""}`}
          hidden={active !== v.id}
        >
          {v.content}
        </div>
      ))}

      {pendingOpen && (
        <PendingModal
          onClose={() => setPendingOpen(false)}
          onItemsChange={setPending}
        />
      )}
    </div>
  );
}

export default MainArea;
