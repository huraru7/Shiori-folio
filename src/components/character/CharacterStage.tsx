import OrbCore from "../orb/OrbCore";
import "./CharacterStage.css";

// キャラクター表示エリア(2026-08-12、UI/UX改善指示書7章)。現在はOrbCoreだが、
// 将来Live2Dキャラクターに差し替える計画があるため、独立したコンポーネントとして
// 切り出している。配置(画面下部を基点)の責務だけをここに持たせ、中身の
// 差し替えやすさを保つ。orbStatus(idle/listening/thinking/speaking)の状態管理は
// ストア側にあるため、ここではサイズだけを渡す。
function CharacterStage() {
  return (
    <div className="character-stage">
      <OrbCore sizePx={110} />
    </div>
  );
}

export default CharacterStage;
