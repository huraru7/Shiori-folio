import { useEffect, useRef, useState } from "react";
import { useShioriStore } from "../../store/useShioriStore";
import { useWindowStore } from "../../store/useWindowStore";
import { api } from "../../api/tauri";
import SourceDocumentModal from "./SourceDocumentModal";
import Book from "../library/Book";
import ChunkModal from "../library/ChunkModal";
import { getFlyFromOffset } from "../../lib/library";
import type { KnowledgeResult } from "../../types";
import "./KnowledgePanel.css";

interface Batch {
  key: number;
  items: KnowledgeResult[];
  timestamp: number;
}

// セッション内で貯め続ける履歴の上限(件数が際限なく増えないための簡易な打ち切り)。
const MAX_HISTORY_BATCHES = 20;

// ナレッジウィンドウ(2026-08-12、デスクトップ型ウィンドウシステムの本実装3-1)。
// この会話セッション中に参照された記録を時系列で積み上げて表示する。
// 明示的な検索(search_knowledge)・identity_guard・常時バックグラウンド検索
// (passive recall)のいずれかで参考情報が使われるたびにAPI側からsourcesが返り、
// 新しいバッチとして履歴の先頭に追加される(直前の内容を上書きしない)。
// 直近のバッチだけ「たった今」、それより前はまとめて「少し前」として区切る
// (モックアップも2区分のみのため、単純さを優先している)。
//
// ツール実行中(orbStatus==="thinking"かつツールが動いている間)は「…集めています」
// を履歴の上に表示し、結果が届いたら本が集まってくる演出(gather/settleGlow)を
// 直近バッチのみ再生する。
function KnowledgePanel() {
  const knowledgeResults = useShioriStore((s) => s.knowledgeResults);
  const activeTool = useShioriStore((s) => s.activeTool);
  const orbStatus = useShioriStore((s) => s.orbStatus);
  const openWindow = useWindowStore((s) => s.openWindow);

  // 本棚→チャンクモーダル→原本ビューの2階層構成。selectedはチャンクモーダル、
  // sourceOpenはその上に重ねて開く原本ビューの開閉を表す(「抜粋に戻る」で
  // sourceOpenだけ閉じ、チャンクモーダルは残る)。
  const [selected, setSelected] = useState<KnowledgeResult | null>(null);
  const [sourceOpen, setSourceOpen] = useState(false);
  const [content, setContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [history, setHistory] = useState<Batch[]>([]);
  const prevResultsRef = useRef<KnowledgeResult[]>(knowledgeResults);
  const nextKeyRef = useRef(0);

  useEffect(() => {
    if (knowledgeResults !== prevResultsRef.current) {
      prevResultsRef.current = knowledgeResults;
      if (knowledgeResults.length > 0) {
        const key = nextKeyRef.current++;
        setHistory((h) => [{ key, items: knowledgeResults, timestamp: Date.now() }, ...h].slice(0, MAX_HISTORY_BATCHES));
        openWindow("knowledge");
      }
    }
  }, [knowledgeResults, openWindow]);

  // ツールが実行中(結果がまだ届いていない)の間だけ、収集中の表示を出す。
  const isCollecting = orbStatus === "thinking" && activeTool !== "idle";
  useEffect(() => {
    if (isCollecting) {
      openWindow("knowledge");
    }
  }, [isCollecting, openWindow]);

  const handleOpenChunk = (result: KnowledgeResult) => {
    setSelected(result);
    setSourceOpen(false);
  };

  const handleOpenSource = () => {
    if (!selected) return;
    setSourceOpen(true);
    setContent(null);
    setError(null);
    api
      .getSourceDocument(selected.sourceCategory, selected.source)
      .then(setContent)
      .catch((err) => setError(String(err)));
  };

  const totalCount = history.reduce((sum, b) => sum + b.items.length, 0);

  return (
    <div className="knowledge-panel">
      {history.length > 0 && (
        <div className="knowledge-panel__header">
          <span>この会話で参照した記録</span>
          <span>{totalCount}件</span>
        </div>
      )}

      {isCollecting && <div className="knowledge-panel__collecting">…集めています</div>}

      {!isCollecting && history.length === 0 && (
        <p className="knowledge-panel__empty">
          会話の中で参考情報が使われると、ここに時系列で積み上がっていきます。
        </p>
      )}

      {history.map((batch, idx) => (
        <div className="knowledge-panel__batch" key={batch.key}>
          <div className="knowledge-panel__time-label">{idx === 0 ? "たった今" : "少し前"}</div>
          <div className="knowledge-panel__grid">
            {batch.items.map((r, i) => (
              <Book
                key={r.id}
                title={r.heading}
                sourceCategory={r.sourceCategory}
                size="md"
                onClick={() => handleOpenChunk(r)}
                gathering={idx === 0}
                gatherDelayMs={i * 110}
                flyFrom={getFlyFromOffset(i)}
              />
            ))}
          </div>
        </div>
      ))}

      {selected && (
        <ChunkModal
          result={selected}
          onClose={() => setSelected(null)}
          onOpenSource={handleOpenSource}
        />
      )}

      {selected && sourceOpen && (
        <SourceDocumentModal
          title={selected.source}
          heading={selected.heading}
          content={content}
          error={error}
          onClose={() => setSourceOpen(false)}
        />
      )}
    </div>
  );
}

export default KnowledgePanel;
