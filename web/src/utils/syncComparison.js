function normalizeNewlines(text) {
  return text.replace(/\r\n?/g, "\n");
}

export function describeSyncDifference(local, remote) {
  if (local === remote) return "The displayed text is identical. Sync again to check whether this conflict has cleared.";
  if (normalizeNewlines(local) === normalizeNewlines(remote)) {
    return "Only line endings differ (Windows CRLF, LF, or CR).";
  }
  if (local.replace(/\s/g, "") === remote.replace(/\s/g, "")) {
    return "Only whitespace differs. Spaces and line breaks can affect formatting; review before choosing a version.";
  }
  return "The contents differ. Show whitespace to reveal spaces, tabs, and line endings.";
}

export function describeSyncText(text) {
  const crlf = (text.match(/\r\n/g) ?? []).length;
  const lf = (text.match(/\n/g) ?? []).length - crlf;
  const cr = (text.match(/\r/g) ?? []).length - crlf;
  const endings = [[crlf, "CRLF"], [lf, "LF"], [cr, "CR"]]
    .filter(([count]) => count > 0)
    .map(([count, label]) => `${count} ${label}`);
  const finalNewline = /[\r\n]$/.test(text) ? "final newline" : "no final newline";
  return `${new TextEncoder().encode(text).length} bytes · ${endings.join(", ") || "no line breaks"} · ${finalNewline}`;
}

export function revealSyncWhitespace(text) {
  return text.replace(/\r\n|\r|\n| |\t|\u00a0|\ufeff|\u200b/g, (character) => {
    switch (character) {
      case "\r\n": return "[CRLF]\n";
      case "\r": return "[CR]\n";
      case "\n": return "[LF]\n";
      case " ": return "·";
      case "\t": return "[TAB]";
      case "\u00a0": return "[NBSP]";
      case "\ufeff": return "[BOM]";
      case "\u200b": return "[ZWSP]";
      default: return character;
    }
  }) + "[EOF]";
}
