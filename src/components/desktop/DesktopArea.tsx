import { useRef } from "react";
import OsWindow from "./OsWindow";
import KnowledgePanel from "../panels/KnowledgePanel";
import LibraryScreen from "../library/LibraryScreen";
import ControlPanel from "../controlpanel/ControlPanel";
import "./DesktopArea.css";

// デスクトップ型フリースペースの土台(2026-08-12、デスクトップ型ウィンドウ
// システムの本実装)。ナレッジ・図書館・設定の3ウィンドウをここに登録する
// (デバッグは対象外、引き続き独立した全画面表示のまま)。
function DesktopArea() {
  const containerRef = useRef<HTMLDivElement>(null);

  return (
    <div className="desktop-area" ref={containerRef}>
      <OsWindow
        id="knowledge"
        title="ナレッジ"
        defaultX={24}
        defaultY={20}
        defaultWidth={320}
        defaultHeight={300}
        minWidth={220}
        minHeight={180}
        containerRef={containerRef}
      >
        <KnowledgePanel />
      </OsWindow>

      <OsWindow
        id="library"
        title="詩織の図書館"
        defaultX={60}
        defaultY={40}
        defaultWidth={460}
        defaultHeight={420}
        minWidth={300}
        minHeight={240}
        containerRef={containerRef}
        fillBody
      >
        <LibraryScreen />
      </OsWindow>

      <OsWindow
        id="settings"
        title="設定"
        defaultX={100}
        defaultY={60}
        defaultWidth={480}
        defaultHeight={420}
        minWidth={320}
        minHeight={260}
        containerRef={containerRef}
        fillBody
      >
        <ControlPanel />
      </OsWindow>
    </div>
  );
}

export default DesktopArea;
