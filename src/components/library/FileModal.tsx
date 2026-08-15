import { getCategoryMeta, hexToRgba } from "../../lib/library";
import type { LibraryFile } from "../../types";
import "./ChunkModal.css";

interface Props {
  file: LibraryFile;
  callNo: string;
  title: string;
  onClose: () => void;
  onOpenSource: () => void;
}

// 本を開いたときのモーダル(Phase 7、1冊=1ファイルの表示単位への変更に伴い
// ChunkModalを置き換え)。チャンクの本文は合成せず、そのファイル内の見出し
// 一覧だけを見せる(「棚の場所を教えるだけで中身を合成しない」という
// search_libraryの設計方針、詩織Ver2.0設計指示書v3、9〜10章と揃える)。
// 「この記録の出どころ」からクリックすると、元ファイル全体を表示する
// 原本ビュー(既存のSourceDocumentModal)へ遷移できる。
function FileModal({ file, callNo, title, onClose, onOpenSource }: Props) {
  const meta = getCategoryMeta(file.sourceCategory);
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
            <div className="chunk-modal__title">{title}</div>
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
          {file.headings.length > 0 ? (
            <ul className="chunk-modal__headings">
              {file.headings.map((heading) => (
                <li key={heading}>{heading}</li>
              ))}
            </ul>
          ) : (
            <p className="chunk-modal__text">見出しが見つかりませんでした。</p>
          )}
          <h4>この記録の出どころ</h4>
          <p>
            <span className="chunk-modal__source-link" onClick={onOpenSource}>
              {file.source}
            </span>
          </p>
        </div>
      </div>
    </div>
  );
}

export default FileModal;
