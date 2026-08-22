import { useEffect, useState } from "react";
import { api } from "../../api/tauri";
import { useWindowStore } from "../../store/useWindowStore";
import type { PendingItems } from "../../types";
import PendingModal from "./PendingModal";
import "./Dock.css";

interface Props {
  // デバッグは今回もウィンドウ化の対象外(独立した全画面表示のまま)なので、
  // App.tsx側のisDebugOpen状態を切り替えるコールバックだけ受け取る。
  onOpenDebug: () => void;
}

function pendingBadgeCount(items: PendingItems): number {
  // バッジは「未着手」のみを数える(保留中はモーダルの別タブで確認する対象で、
  // 対応不要なものとして未着手カウントから除外する、詩織Ver2.0設計指示書v3、
  // Phase 8の方針)。
  return (
    items.tags.filter((t) => t.status === "pending").length +
    items.projects.filter((p) => p.status === "pending").length +
    items.inbox.filter((i) => i.reviewStatus === null).length
  );
}

// macOSのDockを参考にしたタスクバー。区切り線で2グループに分ける: 左＝会話の
// 流れの中で自動的に開くウィンドウ(現状はナレッジのみ)、右＝常時呼び出せる
// 固定アイコン(図書館・要確認・設定・デバッグ)。図書館・設定はデスクトップ型
// ウィンドウシステムの本実装により、OsWindowとしてフリースペース内に
// 表示される(2026-08-12)。要確認(Phase 8)は他のモーダル(FileModal等)と
// 同様、OsWindow化せず単純なオーバーレイモーダルとして実装している。
function Dock({ onOpenDebug }: Props) {
  const knowledgeWindow = useWindowStore((s) => s.windows["knowledge"]);
  const toggleWindow = useWindowStore((s) => s.toggleWindow);

  const [pendingCount, setPendingCount] = useState(0);
  const [pendingOpen, setPendingOpen] = useState(false);

  useEffect(() => {
    api
      .listPendingItems()
      .then((r) => setPendingCount(pendingBadgeCount(r)))
      .catch(() => {
        // ヘッダー同様、取得失敗時もアイコン自体は表示したいのでバッジを
        // 出さないだけにして無視する。
      });
  }, []);

  return (
    <div className="dock-wrap">
      <div className="dock">
        <div
          className="dock__icon"
          title="ナレッジ(会話中に自動で開きます)"
          onClick={() => toggleWindow("knowledge")}
        >
          🔖
          <span className={`dock__dot${knowledgeWindow?.open ? " dock__dot--active" : ""}`} />
        </div>

        <div className="dock__divider" />

        <div className="dock__icon" title="詩織の図書館" onClick={() => toggleWindow("library")}>
          📚
        </div>
        <div
          className="dock__icon"
          title="全件閲覧"
          onClick={() => toggleWindow("library-browse")}
        >
          🗄
        </div>
        <div className="dock__icon" title="要確認の記録" onClick={() => setPendingOpen(true)}>
          🗂
          {pendingCount > 0 && <span className="dock__badge">{pendingCount}</span>}
        </div>
        <div className="dock__icon" title="デバッグ" onClick={onOpenDebug}>
          🧪
        </div>
        <div className="dock__icon" title="設定" onClick={() => toggleWindow("settings")}>
          ⚙
        </div>
      </div>

      {pendingOpen && (
        <PendingModal
          onClose={() => setPendingOpen(false)}
          onItemsChange={(r) => setPendingCount(pendingBadgeCount(r))}
        />
      )}
    </div>
  );
}

export default Dock;
