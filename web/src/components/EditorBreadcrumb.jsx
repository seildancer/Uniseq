function normalizePageBreadcrumbSegment(segment) {
  return segment.replaceAll("___", " > ");
}

export function breadcrumbItemsForPageId(pageId) {
  if (typeof pageId !== "string") {
    return [];
  }

  if (pageId.startsWith("pages:")) {
    const segments = pageId.slice("pages:".length).split("/").filter(Boolean);
    return ["pages", ...segments.flatMap((segment) => normalizePageBreadcrumbSegment(segment).split(" > ").filter(Boolean))];
  }

  if (pageId.startsWith("stream:")) {
    const [streamName] = pageId.slice("stream:".length).split("/");
    return streamName ? ["streams", streamName] : ["streams"];
  }

  return [];
}

export function breadcrumbItemsForStreamSelection(streamSelection) {
  if (!streamSelection) {
    return [];
  }

  if (streamSelection.kind === "stream_single" && streamSelection.streamName) {
    return ["streams", streamSelection.streamName];
  }

  return ["streams"];
}

export default function EditorBreadcrumb({ items }) {
  if (!Array.isArray(items) || items.length === 0) {
    return null;
  }

  return (
    <div className="editor-breadcrumb" aria-label="Breadcrumb">
      {items.map((item, index) => (
        <span key={`${item}-${index}`} className="editor-breadcrumb-item">
          {index > 0 ? <span className="editor-breadcrumb-separator">{">"}</span> : null}
          <span>{item}</span>
        </span>
      ))}
    </div>
  );
}
