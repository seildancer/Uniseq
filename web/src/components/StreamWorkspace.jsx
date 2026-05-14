import SidebarCalendar from "./SidebarCalendar.jsx";
import StreamDualEditor from "./StreamDualEditor.jsx";
import StreamSingleList from "./StreamSingleList.jsx";
import { PRIMARY_STREAM_LEFT } from "../utils/streamWorkspace.js";

export default function StreamWorkspace({
  streamSelection,
  selectedStreamDate,
  streamNames,
  streamPagesByDate,
  regularPages,
  streamReloadToken,
  diaryBlurEnabled,
  onDiaryBlurToggle,
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
            selectedDate={selectedStreamDate}
            streamPagesByDate={streamPagesByDate}
            pages={regularPages}
            reloadToken={streamReloadToken}
            onNavigate={onNavigatePage}
            onError={onError}
            onRefresh={onRefresh}
            diaryBlurEnabled={diaryBlurEnabled}
          />
        ) : (
          <StreamSingleList
            streamName={streamSelection.streamName}
            selectedDate={selectedStreamDate}
            streamPagesByDate={streamPagesByDate}
            pages={regularPages}
            reloadToken={streamReloadToken}
            onNavigate={onNavigatePage}
            onError={onError}
            onRefresh={onRefresh}
            diaryBlurEnabled={diaryBlurEnabled}
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
              className={`stream-section-title${streamSelection?.kind === "stream_dual" ? " stream-section-title--active" : ""}`}
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
              {streamNames.map((streamName) => {
                const isDiary = streamName === PRIMARY_STREAM_LEFT;

                return (
                  <li key={streamName} className={`stream-list-item${isDiary ? " stream-list-item--with-toggle" : ""}`}>
                    <button
                      type="button"
                      className={`stream-list-btn${
                        streamSelection?.kind === "stream_single" && streamSelection.streamName === streamName
                          ? " stream-list-btn--active"
                          : ""
                      }${isDiary ? " stream-list-btn--with-toggle" : ""}`}
                      onClick={() => onSelectStreamSingle(streamName, selectedStreamDate)}
                    >
                      {streamName}
                    </button>
                    {isDiary ? (
                      <button
                        type="button"
                        className={`stream-blur-toggle${diaryBlurEnabled ? " stream-blur-toggle--active" : ""}`}
                        aria-pressed={diaryBlurEnabled}
                        title={diaryBlurEnabled ? "Diary blur is on" : "Diary blur is off"}
                        onClick={onDiaryBlurToggle}
                      >
                        blur
                      </button>
                    ) : null}
                  </li>
                );
              })}
            </ul>
          ) : null}
        </div>
        {pageSidebarContent}
      </aside>

      {streamSelection ? (
        <section className="editor-panel editor-panel--stream">
          {streamEditor}
        </section>
      ) : fallbackEditor}
    </>
  );
}
