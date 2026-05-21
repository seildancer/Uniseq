import pageLeafName from "./pageLeafName.js";

export function pageRefBody(pageId) {
  if (typeof pageId !== "string") {
    return "";
  }

  if (pageId.startsWith("pages:")) {
    return pageId.slice("pages:".length);
  }

  return "";
}

export function pageRefLabel(page) {
  if (!page || typeof page !== "object") {
    return "";
  }

  return pageRefBody(page.page_id) || page.title || pageLeafName(page.page_id) || page.page_id || "";
}

export function pageMatchesRefText(page, refText) {
  if (!page || typeof refText !== "string") {
    return false;
  }

  const normalizedRefText = refText.trim();
  if (!normalizedRefText) {
    return false;
  }

  return (
    pageRefBody(page.page_id) === normalizedRefText
    || pageLeafName(page.page_id) === normalizedRefText
    || page.page_id === normalizedRefText
    || page.title === normalizedRefText
  );
}
