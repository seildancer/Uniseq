import { useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { PRIMARY_STREAM_LEFT, PRIMARY_STREAM_RIGHT, streamPageExists, streamPageId } from "../utils/streamWorkspace.js";

export default function StreamDualEditor({
  dateName,
  streamPagesByDate,
  pages,
  reloadToken,
  onNavigate,
  onError,
  onRefresh,
}) {
  const [mobileTab, setMobileTab] = useState(PRIMARY_STREAM_LEFT);

  const diaryPageId = streamPageExists(streamPagesByDate, dateName, PRIMARY_STREAM_LEFT)
    ? streamPageId(PRIMARY_STREAM_LEFT, dateName)
    : null;
  const journalsPageId = streamPageExists(streamPagesByDate, dateName, PRIMARY_STREAM_RIGHT)
    ? streamPageId(PRIMARY_STREAM_RIGHT, dateName)
    : null;

  return (
    <div className="stream-dual-wrap">
      <div className="stream-dual-tabs">
        <button
          type="button"
          className={`stream-dual-tab${mobileTab === PRIMARY_STREAM_LEFT ? " stream-dual-tab--active" : ""}`}
          onClick={() => setMobileTab(PRIMARY_STREAM_LEFT)}
        >
          {PRIMARY_STREAM_LEFT}
        </button>
        <button
          type="button"
          className={`stream-dual-tab${mobileTab === PRIMARY_STREAM_RIGHT ? " stream-dual-tab--active" : ""}`}
          onClick={() => setMobileTab(PRIMARY_STREAM_RIGHT)}
        >
          {PRIMARY_STREAM_RIGHT}
        </button>
      </div>

      <div className="stream-dual-pane">
        <div className={`stream-dual-panel${mobileTab !== PRIMARY_STREAM_LEFT ? " stream-dual-panel--hidden-mobile" : ""}`}>
          <p className="stream-panel-label">{PRIMARY_STREAM_LEFT}</p>
          <StreamSingleEditor
            key={`${PRIMARY_STREAM_LEFT}/${dateName}`}
            streamName={PRIMARY_STREAM_LEFT}
            dateName={dateName}
            existingPageId={diaryPageId}
            pages={pages}
            reloadToken={reloadToken}
            onNavigate={onNavigate}
            onError={onError}
            onRefresh={onRefresh}
          />
        </div>
        <div className={`stream-dual-panel${mobileTab !== PRIMARY_STREAM_RIGHT ? " stream-dual-panel--hidden-mobile" : ""}`}>
          <p className="stream-panel-label">{PRIMARY_STREAM_RIGHT}</p>
          <StreamSingleEditor
            key={`${PRIMARY_STREAM_RIGHT}/${dateName}`}
            streamName={PRIMARY_STREAM_RIGHT}
            dateName={dateName}
            existingPageId={journalsPageId}
            pages={pages}
            reloadToken={reloadToken}
            onNavigate={onNavigate}
            onError={onError}
            onRefresh={onRefresh}
          />
        </div>
      </div>
    </div>
  );
}
