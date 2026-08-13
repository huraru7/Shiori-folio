import { useState } from "react";
import { useShioriStore } from "../store/useShioriStore";
import "./VoiceBar.css";

// テキスト入力欄(クリック/タイプでも操作可能)。マイクボタンは状態表示用で、
// 実際の録音トリガーはホットキーCtrl+Alt+Sとも共有するtoggleRecordingを呼ぶ。
// 送信処理自体はstore.processUserTextに集約し、ホットキー経由の文字起こし結果と
// 同じ経路(send_message呼び出し・TTS再生)に収束させる。
function VoiceBar() {
  const [text, setText] = useState("");
  const isRecording = useShioriStore((s) => s.isRecording);
  const toggleRecording = useShioriStore((s) => s.toggleRecording);
  const processUserText = useShioriStore((s) => s.processUserText);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = text.trim();
    if (!trimmed) return;
    setText("");
    await processUserText(trimmed);
  }

  return (
    <form className="voice-bar" onSubmit={handleSubmit}>
      <button
        type="button"
        className={`voice-bar__mic${isRecording ? " voice-bar__mic--active" : ""}`}
        onClick={() => toggleRecording()}
        aria-label="録音の開始・停止"
      >
        🎙
      </button>
      <input
        className="voice-bar__input"
        value={text}
        onChange={(e) => setText(e.currentTarget.value)}
        placeholder="話しかける、または入力してください..."
      />
      <button type="submit" className="voice-bar__submit">
        送る
      </button>
    </form>
  );
}

export default VoiceBar;
