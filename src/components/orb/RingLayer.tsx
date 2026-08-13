import type { OrbStatus } from "../../types";
import "./RingLayer.css";

interface RingLayerProps {
  color: string;
  status: OrbStatus;
  sizePx: number;
  opacity: number;
  delaySeconds: number;
}

// 既存モックアップのring/r1-r3に相当する1本のリング
function RingLayer({ color, status, sizePx, opacity, delaySeconds }: RingLayerProps) {
  return (
    <div
      className={`ring-layer ring-layer--${status}`}
      style={{
        width: sizePx,
        height: sizePx,
        borderColor: color,
        opacity,
        animationDelay: `${delaySeconds}s`,
      }}
    />
  );
}

export default RingLayer;
