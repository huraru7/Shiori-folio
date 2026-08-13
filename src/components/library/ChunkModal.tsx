import { getCallNo, getCategoryMeta, hexToRgba } from "../../lib/library";
import type { KnowledgeResult } from "../../types";
import "./ChunkModal.css";

interface Props {
  result: KnowledgeResult;
  onClose: () => void;
  onOpenSource: () => void;
}

// 本を開いたときのチャンクモーダル(2026-08-12、図書館ビジョン統合仕様書3-3)。
// 分類番号・見出し・カテゴリタグ・チャンクの内容(全文)を表示する。「この記録の
// 出どころ」からクリックすると、元ファイル全体を表示する原本ビュー(3-4、実体は
// 既存のSourceDocumentModal)へ遷移できる(本棚→チャンクモーダル→原本ビューの
// 2階層構成)。
function ChunkModal({ result, onClose, onOpenSource }: Props) {
  const meta = getCategoryMeta(result.sourceCategory);
  const callNo = getCallNo(result.id, result.sourceCategory);
  const tagStyle = {
    background: hexToRgba(meta.hex, 0.12),
    borderColor: hexToRgba(meta.hex, 0.3),
    color: meta.hex,
  };

  return (
    <div className="chunk-modal__backdrop" onClick={onClose}>
      <div className="chunk-modal" onClick={(e) => e.stopPropagation()}>
        <div className="chunk-modal__header">
          <div>
            <div className="chunk-modal__callno" style={{ color: meta.hex }}>
              {callNo}
            </div>
            <div className="chunk-modal__title">{result.heading}</div>
          </div>
          <div className="chunk-modal__header-right">
            <span className="chunk-modal__tag" style={tagStyle}>
              {meta.label}
            </span>
            <button className="chunk-modal__close" onClick={onClose} aria-label="閉じる">
              ×
            </button>
          </div>
        </div>
        <div className="chunk-modal__body">
          <p className="chunk-modal__text">{result.text}</p>
          <h4>この記録の出どころ</h4>
          <p>
            <span className="chunk-modal__source-link" onClick={onOpenSource}>
              {result.source}
            </span>
          </p>
        </div>
      </div>
    </div>
  );
}

export default ChunkModal;
