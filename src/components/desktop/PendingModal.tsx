import { useEffect, useState } from "react";
import { api } from "../../api/tauri";
import type { PendingItems } from "../../types";
import "./PendingModal.css";

interface Props {
  onClose: () => void;
  // 一覧が変化するたび(初回取得・承認/却下/保留の反映後)に呼ばれる。
  // Dock側のバッジ件数をモーダルを開いたまま最新に保つため。
  onItemsChange: (items: PendingItems) => void;
}

type Tab = "pending" | "deferred";

const EMPTY: PendingItems = { tags: [], projects: [], inbox: [] };

// 要確認UI(Phase 8、詩織Ver2.0設計指示書v3)のモーダル本体。tags.yaml/
// projects.yamlのpending/deferredなエントリ、および00-inboxの未レビュー記録を
// 一覧表示し、承認(confirm)/却下(reject)/保留(defer)を選べる。
// 「未着手」「保留中」の2タブに分け、保留中タブでも改めて承認/却下できる。
function PendingModal({ onClose, onItemsChange }: Props) {
  const [items, setItems] = useState<PendingItems>(EMPTY);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("pending");
  const [busyKey, setBusyKey] = useState<string | null>(null);

  const refresh = () => {
    setLoading(true);
    api
      .listPendingItems()
      .then((r) => {
        setItems(r);
        onItemsChange(r);
        setError(null);
      })
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    refresh();
    // refresh/onItemsChangeは呼び出しのたびに新しい参照になるため依存配列には
    // 含めない(初回マウント時のみ実行すればよい)。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const runAction = async (key: string, run: () => Promise<void>) => {
    setBusyKey(key);
    try {
      await run();
      refresh();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusyKey(null);
    }
  };

  const tags = items.tags.filter((t) => t.status === tab);
  const projects = items.projects.filter((p) => p.status === tab);
  const inbox = items.inbox.filter((i) => (tab === "pending" ? i.reviewStatus === null : i.reviewStatus === "deferred"));

  const totalInTab = tags.length + projects.length + inbox.length;

  return (
    <div className="pending-modal__backdrop" onClick={onClose}>
      <div className="pending-modal" onClick={(e) => e.stopPropagation()}>
        <div className="pending-modal__header">
          <div className="pending-modal__title">要確認の記録</div>
          <button className="pending-modal__close" onClick={onClose} aria-label="閉じる">
            ×
          </button>
        </div>

        <div className="pending-modal__tabs">
          <button
            className={`pending-modal__tab ${tab === "pending" ? "pending-modal__tab--active" : ""}`}
            onClick={() => setTab("pending")}
          >
            未着手({items.tags.filter((t) => t.status === "pending").length +
              items.projects.filter((p) => p.status === "pending").length +
              items.inbox.filter((i) => i.reviewStatus === null).length})
          </button>
          <button
            className={`pending-modal__tab ${tab === "deferred" ? "pending-modal__tab--active" : ""}`}
            onClick={() => setTab("deferred")}
          >
            保留中({items.tags.filter((t) => t.status === "deferred").length +
              items.projects.filter((p) => p.status === "deferred").length +
              items.inbox.filter((i) => i.reviewStatus === "deferred").length})
          </button>
        </div>

        <div className="pending-modal__body">
          {loading && <p className="pending-modal__status">読み込んでいます…</p>}
          {error && <p className="pending-modal__status">{error}</p>}
          {!loading && !error && totalInTab === 0 && (
            <p className="pending-modal__status">該当する記録はありません。</p>
          )}

          {tags.length > 0 && (
            <>
              <div className="pending-modal__section-label">新規タグ</div>
              {tags.map((t) => {
                const key = `tag:${t.canonical}`;
                return (
                  <div className="pending-modal__item" key={key}>
                    <div className="pending-modal__item-main">
                      <div className="pending-modal__item-title">{t.canonical}</div>
                    </div>
                    <div className="pending-modal__item-actions">
                      <button
                        className="pending-modal__action pending-modal__action--confirm"
                        disabled={busyKey === key}
                        onClick={() => runAction(key, () => api.resolvePendingTag(t.canonical, "confirm"))}
                      >
                        承認
                      </button>
                      <button
                        className="pending-modal__action"
                        disabled={busyKey === key}
                        onClick={() => runAction(key, () => api.resolvePendingTag(t.canonical, "defer"))}
                      >
                        保留
                      </button>
                      <button
                        className="pending-modal__action pending-modal__action--reject"
                        disabled={busyKey === key}
                        onClick={() => runAction(key, () => api.resolvePendingTag(t.canonical, "reject"))}
                      >
                        却下
                      </button>
                    </div>
                  </div>
                );
              })}
            </>
          )}

          {projects.length > 0 && (
            <>
              <div className="pending-modal__section-label">新規プロジェクト</div>
              {projects.map((p) => {
                const key = `project:${p.id}`;
                return (
                  <div className="pending-modal__item" key={key}>
                    <div className="pending-modal__item-main">
                      <div className="pending-modal__item-title">{p.id}</div>
                    </div>
                    <div className="pending-modal__item-actions">
                      <button
                        className="pending-modal__action pending-modal__action--confirm"
                        disabled={busyKey === key}
                        onClick={() => runAction(key, () => api.resolvePendingProject(p.id, "confirm"))}
                      >
                        承認
                      </button>
                      <button
                        className="pending-modal__action"
                        disabled={busyKey === key}
                        onClick={() => runAction(key, () => api.resolvePendingProject(p.id, "defer"))}
                      >
                        保留
                      </button>
                      <button
                        className="pending-modal__action pending-modal__action--reject"
                        disabled={busyKey === key}
                        onClick={() => runAction(key, () => api.resolvePendingProject(p.id, "reject"))}
                      >
                        却下
                      </button>
                    </div>
                  </div>
                );
              })}
            </>
          )}

          {inbox.length > 0 && (
            <>
              <div className="pending-modal__section-label">保存できなかった記録(inbox)</div>
              {inbox.map((i) => {
                const key = `inbox:${i.filename}`;
                return (
                  <div className="pending-modal__item" key={key}>
                    <div className="pending-modal__item-main">
                      <div className="pending-modal__item-title">{i.title ?? i.filename}</div>
                      {i.reason && <div className="pending-modal__item-reason">{i.reason}</div>}
                    </div>
                    <div className="pending-modal__item-actions">
                      <button
                        className="pending-modal__action pending-modal__action--confirm"
                        disabled={busyKey === key}
                        onClick={() =>
                          runAction(key, () => api.resolvePendingInboxItem(i.filename, "confirm"))
                        }
                      >
                        承認
                      </button>
                      <button
                        className="pending-modal__action"
                        disabled={busyKey === key}
                        onClick={() =>
                          runAction(key, () => api.resolvePendingInboxItem(i.filename, "defer"))
                        }
                      >
                        保留
                      </button>
                      <button
                        className="pending-modal__action pending-modal__action--reject"
                        disabled={busyKey === key}
                        onClick={() =>
                          runAction(key, () => api.resolvePendingInboxItem(i.filename, "reject"))
                        }
                      >
                        却下
                      </button>
                    </div>
                  </div>
                );
              })}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default PendingModal;
