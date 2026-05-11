export default function pageLeafName(pageId) {
  return pageId.replace(/^(?:pages|stream):/, "");
}
