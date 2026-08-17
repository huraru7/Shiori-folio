// ControlPanel配下の各ページ(SystemStatusPage/VoiceSettingsPage/
// AdvancedSettingsPage/ModelSwitchPage)で共有する小さな表示ヘルパー・入力部品。
// ControlPanel.tsxが933行まで肥大化していた反省(2026-08-17、プロジェクト
// 整合性レビュー)から、ページ単位でファイルを分割した際の共通置き場として作成。

// 使用率に応じた3段階のレベル分け(60%未満=安全、60〜79%=要注意、80%以上=危険)。
// パネル本文の色使いを静かに保つため、しきい値を超えたときだけ色を変える。
export function levelFor(percent: number): "safe" | "caution" | "danger" {
  if (percent >= 80) return "danger";
  if (percent >= 60) return "caution";
  return "safe";
}

// GPU温度用。ノートPCはサーマルスロットリングが起きやすいため80℃を危険の目安にする。
export function levelForTemp(celsius: number): "safe" | "caution" | "danger" {
  if (celsius >= 80) return "danger";
  if (celsius >= 70) return "caution";
  return "safe";
}

export function formatMb(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)}GB`;
  return `${mb}MB`;
}

export function formatGb(gb: number): string {
  return `${gb.toFixed(1)}GB`;
}

export const HISTORY_LENGTH = 200; // 1.5秒間隔ポーリングで約5分ぶん

// 直近の推移を示す簡易スパークライン。SVGのpolylineで最小限の描画に留める。
export function Sparkline({ values, max }: { values: number[]; max: number }) {
  if (values.length < 2) return null;
  const width = 100;
  const height = 24;
  const points = values
    .map((v, i) => {
      const x = (i / (values.length - 1)) * width;
      const y = height - (Math.min(v, max) / max) * height;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <svg className="control-panel__sparkline" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none">
      <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

export function SliderRow({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  return (
    <div className="control-panel__slider-row">
      <div className="control-panel__slider-top">
        <span>{label}</span>
        <span className="control-panel__slider-val">{value.toFixed(2)}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="control-panel__slider-input"
      />
    </div>
  );
}

export function TextRow({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="control-panel__field-row">
      <span className="control-panel__field-label">{label}</span>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="control-panel__text-input"
      />
    </div>
  );
}

export function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="control-panel__field-row">
      <span className="control-panel__field-label">{label}</span>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </div>
  );
}

export function NumberRow({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
}) {
  return (
    <div className="control-panel__field-row">
      <span className="control-panel__field-label">{label}</span>
      <input
        type="number"
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="control-panel__text-input"
      />
    </div>
  );
}
