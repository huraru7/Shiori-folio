import { useShioriStore } from "../../store/useShioriStore";
import { activityConfig } from "./activityConfig";
import RingLayer from "./RingLayer";
import "./OrbCore.css";

interface Props {
  // 一番外側のリングの直径。他のリング・中心円・コンテナのサイズはこれを
  // 基準にした比率で決まる(元のデザイン値220pxを基準比とする)。
  sizePx?: number;
}

// activeTool/orbStatusをストアから購読し、実行中のツールに応じた色と
// ステータス別のアニメーションを適用する。通常時(idle)はラベルを表示しない。
// CharacterStage(2026-08-12)からサイズを渡せるようにし、左ゾーンのような
// 狭いスペースでも縮小して収まるようにしている。
function OrbCore({ sizePx = 220 }: Props) {
  const activeTool = useShioriStore((s) => s.activeTool);
  const orbStatus = useShioriStore((s) => s.orbStatus);
  const { color, label } = activityConfig[activeTool];

  const containerSize = sizePx * (240 / 220);
  const centerSize = sizePx * (60 / 220);

  return (
    <div className="orb-core" style={{ width: containerSize, height: containerSize }}>
      <RingLayer color={color} status={orbStatus} sizePx={sizePx} opacity={0.3} delaySeconds={0} />
      <RingLayer color={color} status={orbStatus} sizePx={sizePx * (170 / 220)} opacity={0.5} delaySeconds={0.2} />
      <RingLayer color={color} status={orbStatus} sizePx={sizePx * (120 / 220)} opacity={0.8} delaySeconds={0.4} />
      <div
        className="orb-core__center"
        style={{ backgroundColor: color, width: centerSize, height: centerSize }}
      />
      {label && <span className="orb-core__label">{label}</span>}
    </div>
  );
}

export default OrbCore;
