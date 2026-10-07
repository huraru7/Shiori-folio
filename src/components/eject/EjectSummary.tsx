import type { EjectEntry, EjectPreview } from "../../types";
import "./Eject.css";

function Entries({ entries }: { entries: EjectEntry[] }) {
  return (
    <ul className="eject__list">
      {entries.map((e) => (
        <li key={e.pid} className="eject__item">
          <span className="eject__name">{e.name}</span>
          <span className="eject__pid">pid {e.pid}</span>
          {e.note && <div className="eject__note">{e.note}</div>}
        </li>
      ))}
    </ul>
  );
}

// 取り外しの確認結果(止める予定のもの・ほかにSSDを開いているもの)の表示。
// 設定画面の取り外しページと、詩織を閉じるときの確認の両方で使う。
function EjectSummary({ preview }: { preview: EjectPreview }) {
  const { plan, otherHolders } = preview;
  return (
    <div className="eject">
      <div className="eject__section">
        <div className="eject__label">止める予定のもの</div>
        {plan.toStop.length === 0 ? (
          <p className="eject__empty">止めるものはありません。</p>
        ) : (
          <Entries entries={plan.toStop} />
        )}
      </div>
      {otherHolders.length > 0 && (
        <div className="eject__section">
          <div className="eject__label">ほかにSSDのファイルを開いているもの(止めません)</div>
          <ul className="eject__list">
            {otherHolders.map(([pid, name]) => (
              <li key={pid} className="eject__item">
                <span className="eject__name">{name}</span>
                <span className="eject__pid">pid {pid}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

export default EjectSummary;
