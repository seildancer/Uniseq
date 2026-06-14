const FENCE_RE = /^```/;

function mapOutsideFencedCode(markdown, transformLines) {
  const lines = markdown.split("\n");
  const result = [];
  let inFence = false;
  let segment = [];

  const flush = () => {
    if (segment.length > 0) {
      result.push(...transformLines(segment));
      segment = [];
    }
  };

  for (const line of lines) {
    if (FENCE_RE.test(line.trimStart())) {
      flush();
      result.push(line);
      inFence = !inFence;
      continue;
    }

    if (inFence) {
      result.push(line);
    } else {
      segment.push(line);
    }
  }

  flush();
  return result.join("\n");
}

export function toStoredLineBreakMarkdown(markdown) {
  return mapOutsideFencedCode(markdown, (lines) =>
    lines.map((line) => line.replace(/\\$/, ""))
  );
}
