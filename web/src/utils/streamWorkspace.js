import pageLeafName from "./pageLeafName.js";

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

export function readStreamName(location) {
  if (!location || typeof location !== "object") {
    return null;
  }

  if ("stream" in location && location.stream?.stream_name) {
    return location.stream.stream_name;
  }

  if ("Stream" in location && location.Stream?.stream_name) {
    return location.Stream.stream_name;
  }

  return null;
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

export function dateHasAnyStreamContent(streamPagesByDate, dateName, streamNames) {
  if (!dateName || !Array.isArray(streamNames) || streamNames.length === 0) {
    return false;
  }

  const streamNamesForDate = streamPagesByDate.get(dateName);
  if (!streamNamesForDate) {
    return false;
  }

  return streamNames.some((streamName) => streamNamesForDate.has(streamName));
}

export function dateHasContentForSelection(
  streamSelection,
  dateName,
  streamPagesByDate,
  dualStreamNames = [],
) {
  if (!streamSelection || !dateName) {
    return false;
  }

  if (streamSelection.kind === "stream_single" && streamSelection.streamName) {
    return dateHasAnyStreamContent(streamPagesByDate, dateName, [streamSelection.streamName]);
  }

  if (streamSelection.kind === "stream_dual") {
    return dateHasAnyStreamContent(streamPagesByDate, dateName, dualStreamNames);
  }

  return false;
}

export function selectionForPageId(pageId, location = null) {
  if (typeof pageId !== "string" || pageId.length === 0) {
    return null;
  }

  const streamName = readStreamName(location);
  if (streamName) {
    const dateName = pageLeafName(pageId);
    return dateName
      ? { kind: "stream_single", streamName, dateName }
      : { kind: "page", pageId };
  }

  if (pageId.startsWith("stream:")) {
    const normalizedPageId = pageId.slice("stream:".length);
    const separatorIndex = normalizedPageId.indexOf("/");
    if (separatorIndex > 0 && separatorIndex < normalizedPageId.length - 1) {
      return {
        kind: "stream_single",
        streamName: normalizedPageId.slice(0, separatorIndex),
        dateName: normalizedPageId.slice(separatorIndex + 1),
      };
    }
  }

  return { kind: "page", pageId };
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

export function orderStreamNamesForDisplay(streamNames, preferredOrder = []) {
  const names = Array.isArray(streamNames) ? [...streamNames] : [];
  const seen = new Set();
  const ordered = [];

  for (const streamName of preferredOrder) {
    if (names.includes(streamName) && !seen.has(streamName)) {
      ordered.push(streamName);
      seen.add(streamName);
    }
  }

  for (const streamName of PRIMARY_STREAM_NAMES) {
    if (names.includes(streamName) && !seen.has(streamName)) {
      ordered.push(streamName);
      seen.add(streamName);
    }
  }

  for (const streamName of names) {
    if (!seen.has(streamName)) {
      ordered.push(streamName);
      seen.add(streamName);
    }
  }

  return ordered;
}

export function isDiaryStream(streamName) {
  return streamName === DIARY_STREAM;
}

export function readDualStreamNames(streamNames, preferredOrder) {
  return orderStreamNamesForDisplay(streamNames, preferredOrder).slice(0, 2);
}
