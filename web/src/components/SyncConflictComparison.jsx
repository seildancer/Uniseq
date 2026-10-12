import { useState } from "react";
import { describeSyncDifference, describeSyncText, revealSyncWhitespace } from "../utils/syncComparison.js";

export default function SyncConflictComparison({ local, remote }) {
  const [showWhitespace, setShowWhitespace] = useState(false);
  return (
    <>
      <p className="modal-hint">{describeSyncDifference(local, remote)}</p>
      <label className="sync-whitespace-toggle">
        <input type="checkbox" checked={showWhitespace} onChange={(event) => setShowWhitespace(event.target.checked)} />
        Show whitespace (· = space; [EOF] = end of file)
      </label>
      <div className="sync-diff-grid">
        {[["Local", local], ["Remote", remote]].map(([label, content]) => (
          <div key={label}>
            <span>{label}</span>
            <small className="modal-hint">{describeSyncText(content)}</small>
            <pre>{showWhitespace ? revealSyncWhitespace(content) : content}</pre>
          </div>
        ))}
      </div>
    </>
  );
}
