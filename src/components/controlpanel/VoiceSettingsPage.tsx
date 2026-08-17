import { useEffect, useState } from "react";
import { api } from "../../api/tauri";
import { SliderRow } from "./shared";

const PREVIEW_TEXT = "こんにちは、詩織です。今の設定で話すとこんな感じになります。";

// 発話パラメータ(速度・ノイズ)を試しながら調整するページ。
// 「試し読み」は保存前の値でその場で再生するだけ(config.jsonには書き込まない)。
// 保存は別ボタンで、piper-plusは呼び出しのたびに引数を渡すだけの設計のため
// サービス再起動は不要(即時反映)。
export function VoiceSettingsPage() {
  const [lengthScale, setLengthScale] = useState(1);
  const [noiseScale, setNoiseScale] = useState(0.667);
  const [noiseW, setNoiseW] = useState(0.8);
  const [loaded, setLoaded] = useState(false);
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    api
      .getConfig()
      .then((config) => {
        setLengthScale(config.lengthScale);
        setNoiseScale(config.noiseScale);
        setNoiseW(config.noiseW);
        setLoaded(true);
      })
      .catch((err) => setErrorMessage(String(err)));
  }, []);

  const handlePreview = async () => {
    setIsPreviewing(true);
    setErrorMessage(null);
    try {
      await api.previewVoice(PREVIEW_TEXT, lengthScale, noiseScale, noiseW);
    } catch (err) {
      setErrorMessage(String(err));
    } finally {
      setIsPreviewing(false);
    }
  };

  const handleSave = async () => {
    setSaveState("saving");
    setErrorMessage(null);
    try {
      await api.setConfig({ lengthScale, noiseScale, noiseW });
      setSaveState("saved");
    } catch (err) {
      setSaveState("error");
      setErrorMessage(String(err));
    }
  };

  return (
    <>
      <div className="control-panel__page-head">
        <h1>音声設定</h1>
        <p>発話パラメータを調整できます。保存すると次の発話から即時反映されます(再起動不要)。</p>
      </div>

      {errorMessage && <div className="control-panel__error">{errorMessage}</div>}

      <div className="control-panel__panel">
        <div className="control-panel__panel-label">発話パラメータ</div>

        <SliderRow
          label="発話速度(length-scale)"
          value={lengthScale}
          min={0.5}
          max={2}
          step={0.05}
          onChange={setLengthScale}
        />
        <SliderRow
          label="声の揺らぎ(noise-scale)"
          value={noiseScale}
          min={0}
          max={1.5}
          step={0.01}
          onChange={setNoiseScale}
        />
        <SliderRow
          label="音素幅ノイズ(noise-w)"
          value={noiseW}
          min={0}
          max={1.5}
          step={0.01}
          onChange={setNoiseW}
        />

        <div className="control-panel__actions">
          <button className="control-panel__btn" onClick={handlePreview} disabled={!loaded || isPreviewing}>
            {isPreviewing ? "再生中..." : "試し読み再生"}
          </button>
          <button
            className="control-panel__btn control-panel__btn--primary"
            onClick={handleSave}
            disabled={!loaded || saveState === "saving"}
          >
            {saveState === "saving" ? "保存中..." : "保存して反映"}
          </button>
        </div>

        {saveState === "saved" && (
          <div className="control-panel__note control-panel__note--safe">即時反映されました(再起動は不要です)</div>
        )}
      </div>
    </>
  );
}
