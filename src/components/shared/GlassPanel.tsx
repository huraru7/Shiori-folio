import type { ReactNode } from "react";
import "./GlassPanel.css";

interface GlassPanelProps {
  children: ReactNode;
  className?: string;
}

// グラスモーフィズムの共通枠。全パネルがこれをラップする。
function GlassPanel({ children, className }: GlassPanelProps) {
  return (
    <div className={`glass-panel${className ? ` ${className}` : ""}`}>
      {children}
    </div>
  );
}

export default GlassPanel;
