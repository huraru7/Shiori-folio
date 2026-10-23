import ActivityIndicator from "../ActivityIndicator";
import VoiceBar from "../VoiceBar";
import ConversationLog from "../ConversationLog";
import CharacterStage from "../character/CharacterStage";
import ExternalModeNotice from "../ExternalModeNotice";
import KnowledgePanel from "../panels/KnowledgePanel";
import { useAppModeStore } from "../../store/useAppModeStore";
import "./ConversationScreen.css";

// 会話の画面(詩織Ver6.0の段階2で、左の会話の場所を列の画面の1つにした)。
// 中身は、それまでの左の場所(会話の記録、キャラクター、入力欄)のまま。ほかの画面に移っても、
// 隠すだけで出したままにする(録音中や返事の途中でも止まらない)。
// 右側には、この会話で参照された記録(ナレッジ)を並べる(ナレッジの画面を会話に入れた)。
function ConversationScreen() {
  const mode = useAppModeStore((s) => s.mode);
  const switchingMode = useAppModeStore((s) => s.switching);
  // 外部AIモードのときと、会話モードへ戻して会話用AIを起動している最中は、会話の代わりに案内を出す。
  const showExternalNotice = mode === "external" || (mode === "conversation" && switchingMode);

  return (
    <div className="conversation-screen" data-conversation-mode={showExternalNotice ? "external" : "conversation"}>
      <div className="conversation-screen__talk">
        {showExternalNotice ? (
          <ExternalModeNotice />
        ) : (
          <>
            <ConversationLog />
            <div className="conversation-screen__bottom">
              <ActivityIndicator />
              <CharacterStage />
              <VoiceBar />
            </div>
          </>
        )}
      </div>
      <aside className="conversation-screen__knowledge" aria-label="ナレッジ">
        <div className="conversation-screen__knowledge-title">ナレッジ</div>
        <KnowledgePanel />
      </aside>
    </div>
  );
}

export default ConversationScreen;
