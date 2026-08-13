import { useWindowStore } from "../../store/useWindowStore";
import "./Dock.css";

interface Props {
  // デバッグは今回もウィンドウ化の対象外(独立した全画面表示のまま)なので、
  // App.tsx側のisDebugOpen状態を切り替えるコールバックだけ受け取る。
  onOpenDebug: () => void;
}

// macOSのDockを参考にしたタスクバー。区切り線で2グループに分ける: 左＝会話の
// 流れの中で自動的に開くウィンドウ(現状はナレッジのみ)、右＝常時呼び出せる
// 固定アイコン(図書館・設定・デバッグ)。図書館・設定はデスクトップ型
// ウィンドウシステムの本実装により、OsWindowとしてフリースペース内に
// 表示される(2026-08-12)。
function Dock({ onOpenDebug }: Props) {
  const knowledgeWindow = useWindowStore((s) => s.windows["knowledge"]);
  const toggleWindow = useWindowStore((s) => s.toggleWindow);

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
        <div className="dock__icon" title="デバッグ" onClick={onOpenDebug}>
          🧪
        </div>
        <div className="dock__icon" title="設定" onClick={() => toggleWindow("settings")}>
          ⚙
        </div>
      </div>
    </div>
  );
}

export default Dock;
