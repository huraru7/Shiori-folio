import { useEffect, useState } from "react";
import { useMainViewStore } from "../../store/useMainViewStore";
import { useSidebarItems } from "../../store/useSidebarStore";
import ExtensionFrame from "../extensions/ExtensionFrame";
import "./ToolFrame.css";

// 小道具の枠(詩織Ver6.0の段階2)。いまの画面を切り替えずに、右の端の小さな枠で小道具を開く。
// 開ける枠は1つで、別の小道具を押すと入れ替わる。閉じても中身の状態が残るよう、一度開いた小道具は
// 隠すだけで出したままにする(段階3のタイマーが、閉じても知らせるための下準備)。
function ToolFrame() {
  const openTool = useMainViewStore((s) => s.openTool);
  const closeTool = useMainViewStore((s) => s.closeTool);
  const { all } = useSidebarItems();
  const tools = all.filter((i) => i.group === "tool" && i.ext);
  // 一度でも開いた小道具の名前(出したままにする)
  const [opened, setOpened] = useState<string[]>([]);

  useEffect(() => {
    if (openTool && !opened.includes(openTool)) setOpened((o) => [...o, openTool]);
  }, [openTool, opened]);

  const current = tools.find((t) => t.ext?.name === openTool);

  return (
    <aside className="tool-frame" hidden={!current} aria-label={current?.label ?? "小道具"}>
      <div className="tool-frame__bar">
        <span className="tool-frame__title">{current?.label}</span>
        <button type="button" className="tool-frame__close" data-ripple onClick={closeTool} aria-label="小道具を閉じる">
          閉じる
        </button>
      </div>
      <div className="tool-frame__body">
        {tools
          .filter((t) => t.ext && opened.includes(t.ext.name))
          .map((t) => (
            <div key={t.id} className="tool-frame__slot" data-tool-name={t.ext!.name} hidden={t.ext!.name !== openTool}>
              <ExtensionFrame name={t.ext!.name} entry={t.ext!.entry} title={t.label} />
            </div>
          ))}
      </div>
    </aside>
  );
}

export default ToolFrame;
