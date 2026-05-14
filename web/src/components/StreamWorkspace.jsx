import SidebarCalendar from "./SidebarCalendar.jsx";
import StreamDualEditor from "./StreamDualEditor.jsx";
import StreamSingleEditor from "./StreamSingleEditor.jsx";
import { formatDateLabel } from "../utils/streamDates.js";
import { streamPageExists, streamPageId } from "../utils/streamWorkspace.js";

export default function StreamWorkspace({
  streamSelection,
  selectedStreamDate,
  streamNames,
  streamPagesByDate,
  regularPages,
  streamReloadToken,
  pageSidebarContent,
  fallbackEditor,
  onSelectStreamDual,
  onSelectStreamSingle,
  onNavigatePage,
  onError,
  onRefresh,
}) {
  const streamEditor = streamSelection
    ? (
        streamSelection.kind === "stream_dual" ? (
          <StreamDualEditor
            key={selectedStreamDate}
            dateName={selectedStreamDate}
            streamPagesByDate={streamPagesByDate}
            pages={regularPages}
            reloadToken={streamReloadToken}
            onNavigate={onNavigatePage}
            onError={onError}
            onRefresh={onRefresh}
          />
        ) : (
          <StreamSingleEditor
            key={`${streamSelection.streamName}/${selectedStreamDate}`}
            streamName={streamSelection.streamName}
            dateName={selectedStreamDate}
            existingPageId={
              streamPageExists(streamPagesByDate, selectedStreamDate, streamSelection.streamName)
                ? streamPageId(streamSelection.streamName, selectedStreamDate)
                : null
            }
            pages={regularPages}
            reloadToken={streamReloadToken}
            onNavigate={onNavigatePage}
            onError={onError}
            onRefresh={onRefresh}
          />
        )
      )
    : null;

  return (
    <>
      <aside className="workspace-sidebar">
        <div className="sidebar-section sidebar-section--streams">
          <div className="section-heading">
            <button
              type="button"
              className={`stream-section-title${streamSelection.kind === "stream_dual" ? " stream-section-title--active" : ""}`}
              onClick={() => onSelectStreamDual(selectedStreamDate)}
            >
              Streams
            </button>
          </div>

          <SidebarCalendar
            selectedDate={selectedStreamDate}
            streamPagesByDate={streamPagesByDate}
            onSelectDate={onSelectStreamDual}
          />

          {streamNames.length > 0 ? (
            <ul className="stream-list">
              {streamNames.map((streamName) => (
                <li key={streamName} className="stream-list-item">
                  <button
                    type="button"
                    className={`stream-list-btn${
                      streamSelection.kind === "stream_single" && streamSelection.streamName === streamName
                        ? " stream-list-btn--active"
                        : ""
                    }`}
                    onClick={() => onSelectStreamSingle(streamName, selectedStreamDate)}
                  >
                    {streamName}
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
        {pageSidebarContent}
      </aside>

      {streamSelection ? (
        <section className="editor-panel">
          <p className="eyebrow">
            {streamSelection.kind === "stream_single" ? streamSelection.streamName : "Streams"}
          </p>
          <h1 className="editor-title-static">{formatDateLabel(selectedStreamDate)}</h1>
          {streamEditor}
        </section>
      ) : fallbackEditor}
    </>
  );
}
