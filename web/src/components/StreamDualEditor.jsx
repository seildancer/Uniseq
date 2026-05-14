import { useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";

export default function StreamDualEditor({
  dateName,
  streamPagesByDate,
  pages,
  onNavigate,
  onError,
  onRefresh,
}) {
  const [mobileTab, setMobileTab] = useState("diary");

  const streamNamesForDate = streamPagesByDate.get(dateName) ?? [];
  const diaryPageId = streamNamesForDate.includes("diary")
    ? `stream:diary/${dateName}`
    : null;
  const journalsPageId = streamNamesForDate.includes("journals")
    ? `stream:journals/${dateName}`
    : null;

  return (
    <div className="stream-dual-wrap">
      <div className="stream-dual-tabs">
        <button
          type="button"
          className={`stream-dual-tab${mobileTab === "diary" ? " stream-dual-tab--active" : ""}`}
          onClick={() => setMobileTab("diary")}
        >
          diary
        </button>
        <button
          type="button"
          className={`stream-dual-tab${mobileTab === "journals" ? " stream-dual-tab--active" : ""}`}
          onClick={() => setMobileTab("journals")}
        >
          journals
        </button>
      </div>

      <div className="stream-dual-pane">
        <div className={`stream-dual-panel${mobileTab !== "diary" ? " stream-dual-panel--hidden-mobile" : ""}`}>
          <p className="stream-panel-label">diary</p>
          <StreamSingleEditor
            key={`diary/${dateName}`}
            streamName="diary"
            dateName={dateName}
            existingPageId={diaryPageId}
            pages={pages}
            onNavigate={onNavigate}
            onError={onError}
            onRefresh={onRefresh}
          />
        </div>
        <div className={`stream-dual-panel${mobileTab !== "journals" ? " stream-dual-panel--hidden-mobile" : ""}`}>
          <p className="stream-panel-label">journals</p>
          <StreamSingleEditor
            key={`journals/${dateName}`}
            streamName="journals"
            dateName={dateName}
            existingPageId={journalsPageId}
            pages={pages}
            onNavigate={onNavigate}
            onError={onError}
            onRefresh={onRefresh}
          />
        </div>
      </div>
    </div>
  );
}
