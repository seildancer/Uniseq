const FENCE_RE = /^```/;
const WIKILINK_RE = /\\?\[\\?\[([^\]\n]*?)\\?\]\\?\]/g;

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
  return mapOutsideFencedCode(markdown, (line) =>
    line
      .replace(WIKILINK_RE, "[[$1]]")
      .replace(/<br\s*\/?>/gi, "")
  );
}
