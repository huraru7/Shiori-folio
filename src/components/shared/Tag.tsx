import type { TagVariant } from "../../types";
import "./Tag.css";

interface TagProps {
  label: string;
  variant: TagVariant;
}

// 感情タグ・ラベル用の共通部品
function Tag({ label, variant }: TagProps) {
  return <span className={`tag tag--${variant}`}>{label}</span>;
}

export default Tag;
