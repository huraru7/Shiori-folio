import { useEffect, useRef } from "react";
import { useShioriStore } from "../store/useShioriStore";
import "./ConversationLog.css";

// messagesストアの内容を表示する会話ログ。VoiceBarの直上に配置し、
// 新しいメッセージが来たら自動で最下部にスクロールする。
// メッセージが0件でも(nullを返さず)コンテナ自体は常に描画する。flex:1の
// このコンテナが無いと、下の「下部エリア」(キャラクター・入力欄)が
// 左ゾーンの上に浮いてしまう(0-2、2026-08-12)。
function ConversationLog() {
  const messages = useShioriStore((s) => s.messages);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length]);

  return (
    <div className="conversation-log">
      {messages.map((m, i) => (
        <div
          key={i}
          className={`conversation-log__bubble conversation-log__bubble--${m.role}`}
        >
          {m.text}
        </div>
      ))}
      <div ref={bottomRef} />
    </div>
  );
}

export default ConversationLog;
