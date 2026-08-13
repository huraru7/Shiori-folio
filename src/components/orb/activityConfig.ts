import type { ActiveTool } from "../../types";

// 単一人格+Function Calling方式では、モードをユーザーが選ぶ概念は存在しない。
// ここでは「今まさに実行中のツール」を表す一時的なアクティビティの見た目
// (色・ラベル)だけを定義する。ラベルが空文字のもの(idle)はUI側で
// 表示しない(通常時は何も出さない、という方針)。
export const activityConfig: Record<
  ActiveTool,
  { color: string; label: string }
> = {
  idle: {
    color: "var(--accent-chat)",
    label: "",
  },
  search_knowledge: {
    color: "var(--accent-search)",
    label: "調べています",
  },
  memo: {
    color: "var(--accent-task)",
    label: "メモしています",
  },
};
