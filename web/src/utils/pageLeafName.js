export default function pageLeafName(pageId) {
  if (typeof pageId !== "string") {
    return "";
  }

  const normalizedPageId = pageId.replace(/^(?:pages|stream):/, "");
  return normalizedPageId.split("/").at(-1) ?? normalizedPageId;
}
