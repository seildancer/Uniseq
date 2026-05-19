const FENCE_RE = /^```/;
const WIKILINK_RE = /\\?\[\\?\[([^\]\n]*?)\\?\]\\?\]/g;
const LIST_MARKER_RE = /^\s*(?:[-*+] |\d+[.)] )/;
const INDENTED_LIST_MARKER_RE = /^\s{2,}(?:[-*+] |\d+[.)] )/;

function mapOutsideFencedCode(markdown, transformLine) {
  const lines = markdown.split("\n");
  let inFence = false;

  return lines
    .map((line) => {
      if (FENCE_RE.test(line.trimStart())) {
        inFence = !inFence;
        return line;
      }
      if (inFence) return line;
      return transformLine(line);
    })
    .join("\n");
}

function unescapeLeadingHashTag(line) {
  return line.replace(/^(\s*(?:[-*+] |\d+[.)] )?)\\#(?=\S)/, "$1#");
}

function collapseListSpacingOutsideFencedCode(markdown) {
  const lines = markdown.split("\n");
  let inFence = false;
  const nextLines = [];

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    if (FENCE_RE.test(line.trimStart())) {
      inFence = !inFence;
      nextLines.push(line);
      continue;
    }

    if (
      !inFence
      && line.trim() === ""
      && LIST_MARKER_RE.test(nextLines[nextLines.length - 1] ?? "")
      && INDENTED_LIST_MARKER_RE.test(lines[index + 1] ?? "")
    ) {
      continue;
    }

    nextLines.push(line);
  }

  return nextLines.join("\n");
}

export function normalizeWikilinksOutsideFencedCode(markdown) {
  return mapOutsideFencedCode(markdown, (line) =>
    line.replace(WIKILINK_RE, "[[$1]]")
  );
}

export function stripBreakOutsideFencedCode(markdown) {
  return mapOutsideFencedCode(markdown, (line) =>
    line.replace(/<br\s*\/?>/gi, "")
  );
}

export function cleanEditorMarkdownForPersistence(markdown) {
  return collapseListSpacingOutsideFencedCode(
    mapOutsideFencedCode(markdown, (line) =>
      unescapeLeadingHashTag(
        line
          .replace(WIKILINK_RE, "[[$1]]")
          .replace(/<br\s*\/?>/gi, "")
      )
    )
  );
}
