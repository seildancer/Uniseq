const FENCE_RE = /^```/;

export function stripBreakOutsideFencedCode(markdown) {
  const lines = markdown.split("\n");
  let inFence = false;

  return lines
    .map((line) => {
      if (FENCE_RE.test(line.trimStart())) {
        inFence = !inFence;
        return line;
      }
      if (inFence) return line;
      return line.replace(/<br\s*\/?>/gi, "");
    })
    .join("\n");
}
