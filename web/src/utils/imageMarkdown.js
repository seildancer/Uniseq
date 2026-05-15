import { convertFileSrc } from "@tauri-apps/api/core";

const STORED_IMAGE_RE =
  /!\[([^\]]*)\]\((\.\.\/assets\/[^)\s]+)\)\{\:height\s+(\d+),\s*\:width\s+(\d+)\}/g;
const EDITOR_IMAGE_RE =
  /!\[([^\]]*)\]\(([^)\s]+)\s+"uniseq-image\|(\d+)\|(\d+)\|([^"]+)"\)/g;

function normalizeWorkspaceRoot(workspaceRoot) {
  return String(workspaceRoot ?? "").replace(/\\/g, "/").replace(/\/+$/, "");
}

function assetAbsolutePath(workspaceRoot, relativeAssetPath) {
  const normalizedRoot = normalizeWorkspaceRoot(workspaceRoot);
  const relativePath = String(relativeAssetPath).replace(/^\.\.\//, "");
  return `${normalizedRoot}/${relativePath}`;
}

export function toEditorMarkdown(markdown, workspaceRoot) {
  if (!workspaceRoot || !markdown.includes("../assets/")) {
    return markdown;
  }

  return markdown.replace(
    STORED_IMAGE_RE,
    (_match, alt, relativeAssetPath, height, width) => {
      const assetSrc = convertFileSrc(assetAbsolutePath(workspaceRoot, relativeAssetPath));
      const relEncoded = encodeURIComponent(relativeAssetPath);
      return `![${alt}](${assetSrc} "uniseq-image|${height}|${width}|${relEncoded}")`;
    },
  );
}

export function toStoredMarkdown(markdown) {
  if (!markdown.includes("uniseq-image|")) {
    return markdown;
  }

  return markdown.replace(
    EDITOR_IMAGE_RE,
    (_match, alt, _displaySrc, height, width, relativeAssetPathEncoded) => {
      const relativeAssetPath = decodeURIComponent(relativeAssetPathEncoded);
      return `![${alt}](${relativeAssetPath}){:height ${height}, :width ${width}}`;
    },
  );
}

export function applyImageSizing(root) {
  if (!root) {
    return;
  }

  for (const image of root.querySelectorAll('img[title^="uniseq-image|"]')) {
    const [, height, width] = image.title.split("|");
    const parsedHeight = Number.parseInt(height, 10);
    const parsedWidth = Number.parseInt(width, 10);
    if (!Number.isFinite(parsedHeight) || !Number.isFinite(parsedWidth) || parsedHeight <= 0 || parsedWidth <= 0) {
      continue;
    }

    image.style.width = `${parsedWidth}px`;
    image.style.maxWidth = "100%";
    image.style.height = "auto";
    image.style.aspectRatio = `${parsedWidth} / ${parsedHeight}`;
    image.removeAttribute("title");
  }
}
