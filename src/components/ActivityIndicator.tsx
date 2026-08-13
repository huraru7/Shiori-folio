import { useShioriStore } from "../store/useShioriStore";
import { activityConfig } from "./orb/activityConfig";
import "./ActivityIndicator.css";

// 「安全なとき・通常時は何も表示しない、意味のある変化のときだけ控えめに
// 見せる」という方針(コントロールパネルの危険度表現と同じ思想)をここでも
// 踏襲する。ツール選択の概念は存在しないため、クリック不可の表示専用。
// search_knowledge/memo実行中だけ控えめなラベルを出し、それ以外
// (idle、passive recallヒット時も含む)は何も表示しない。passive recallの
// 痕跡はKnowledgePanel側で表現するため、ここで重ねて表示する必要はない。
function ActivityIndicator() {
  const activeTool = useShioriStore((s) => s.activeTool);
  const { color, label } = activityConfig[activeTool];

  if (!label) {
    return null;
  }

  return (
    <div className="activity-indicator" style={{ borderColor: color, color }}>
      {label}
    </div>
  );
}

export default ActivityIndicator;
