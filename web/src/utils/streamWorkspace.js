export const DIARY_STREAM = "diary";
export const JOURNALS_STREAM = "journals";
export const PRIMARY_STREAM_LEFT = JOURNALS_STREAM;
export const PRIMARY_STREAM_RIGHT = DIARY_STREAM;
export const PRIMARY_STREAM_NAMES = [PRIMARY_STREAM_LEFT, PRIMARY_STREAM_RIGHT];

export function streamPageId(streamName, dateName) {
  return `stream:${streamName}/${dateName}`;
}

export function streamPageExists(streamPagesByDate, dateName, streamName) {
  return streamPagesByDate.get(dateName)?.has(streamName) ?? false;
}

export function readSelectedStreamDate(selection, lastStreamDate) {
  return selection?.kind && selection.kind !== "page"
    ? selection.dateName
    : lastStreamDate;
}

export function selectionForCalendarDate(selection, dateName) {
  if (selection?.kind === "stream_single" && selection.streamName) {
    return { kind: "stream_single", streamName: selection.streamName, dateName };
  }

  return { kind: "stream_dual", dateName };
}

export function shouldBumpStreamReloadToken(event, hasActiveStreamSelection) {
  if (!hasActiveStreamSelection || !event || typeof event !== "object") {
    return false;
  }

  if (event.type === "workspace_reloaded") {
    return true;
  }

  if (event.type === "pages_changed") {
    return Array.isArray(event.page_ids)
      && event.page_ids.some((pageId) => typeof pageId === "string" && pageId.startsWith("stream:"));
  }

  if (event.type === "page_removed") {
    return typeof event.page_id === "string" && event.page_id.startsWith("stream:");
  }

  return false;
}

export function hasExtraStreams(streamNamesForDate) {
  if (!streamNamesForDate) {
    return false;
  }

  for (const streamName of streamNamesForDate) {
    if (!PRIMARY_STREAM_NAMES.includes(streamName)) {
      return true;
    }
  }

  return false;
}

export function orderStreamNamesForDisplay(streamNames) {
  const names = Array.isArray(streamNames) ? [...streamNames] : [];
  const primaryNames = PRIMARY_STREAM_NAMES.filter((streamName) => names.includes(streamName));
  const extraNames = names.filter((streamName) => !PRIMARY_STREAM_NAMES.includes(streamName));
  return [...primaryNames, ...extraNames];
}

export function isDiaryStream(streamName) {
  return streamName === DIARY_STREAM;
}
