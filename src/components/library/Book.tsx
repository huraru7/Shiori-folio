import type { CSSProperties } from "react";
import type { KnowledgeResult } from "../../types";
import { getBookCategoryClass } from "../../lib/library";
import "./Book.css";

// 本ごとに少しずつ違う方向から飛んでくるように見せるための移動元オフセット
// (--fx/--fy/--fr、Book.cssの@keyframes book-gatherで参照)。CSS変数は
// TypeScript標準の型では表現できないため、Record<string, string>として渡す。
type FlyFromVars = CSSProperties & Record<"--fx" | "--fy" | "--fr", string>;

interface Props {
  result: KnowledgeResult;
  onClick: () => void;
  // 「司書が持ってくる棚」の集まってくる演出用。gatherDelayMsを指定すると、
  // その分だけ遅れてgatherアニメーションが再生される(本ごとの時間差を出すため)。
  gathering?: boolean;
  gatherDelayMs?: number;
  flyFrom?: { fx: string; fy: string; fr: string };
  // ウィンドウ内表示用のサイズ(2026-08-12、デスクトップ型ウィンドウシステムの
  // 本実装)。md=ナレッジウィンドウ(68×92)、sm=図書館ウィンドウ(52×70)。
  // window-contents-concept.htmlのモックアップに合わせ、タイトルのみを表示する
  // (分類番号・カテゴリラベルはクリック後のチャンクモーダルで確認できるため省略)。
  size?: "md" | "sm";
}

// 図書館UI共通の「本」1冊分の見た目。表紙カード方式。
// KnowledgePanel・LibraryScreenで共有する。
function Book({ result, onClick, gathering, gatherDelayMs, flyFrom, size = "md" }: Props) {
  const categoryClass = getBookCategoryClass(result.sourceCategory);

  const style: FlyFromVars | undefined = gathering
    ? {
        animationDelay: `${gatherDelayMs ?? 0}ms`,
        "--fx": flyFrom?.fx ?? "0px",
        "--fy": flyFrom?.fy ?? "-60px",
        "--fr": flyFrom?.fr ?? "-12deg",
      }
    : undefined;

  return (
    <div
      className={`book book--${size} ${categoryClass} ${gathering ? "book--gathering" : ""}`}
      style={style}
      onClick={onClick}
      title={result.heading}
    >
      <div className="book__title">{result.heading}</div>
    </div>
  );
}

export default Book;
