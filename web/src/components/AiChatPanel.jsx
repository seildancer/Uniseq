import ReactMarkdown from "react-markdown";
import breaks from "remark-breaks";

function AiChatMessage({ message }) {
  return (
    <article className={`ai-chat-message ai-chat-message--${message.role}`}>
      <div className="ai-chat-message-label">{message.role === "assistant" ? "AI" : "You"}</div>
      <div className="ai-chat-message-body">
        <ReactMarkdown remarkPlugins={[breaks]}>
          {message.content}
        </ReactMarkdown>
      </div>
    </article>
  );
}

export default function AiChatPanel({
  isOpen,
  presentation,
  previewSummary,
  truncated,
  messages,
  draft,
  apiKey,
  loadingSession,
  sending,
  error,
  onClose,
  onDraftChange,
  onApiKeyChange,
  onSubmit,
}) {
  if (!isOpen) {
    return null;
  }

  const panelClassName = presentation === "mobile"
    ? "ai-chat-panel ai-chat-panel--mobile"
    : "ai-chat-panel ai-chat-panel--desktop";

  return (
    <div
      className={`ai-chat-overlay${presentation === "mobile" ? " ai-chat-overlay--mobile" : ""}`}
      data-no-window-drag="true"
      onClick={onClose}
    >
      <section className={panelClassName} onClick={(event) => event.stopPropagation()}>
        <header className="ai-chat-header">
          <div className="ai-chat-header-copy">
            <strong>AI Chat</strong>
          </div>
          <button
            className="ai-chat-close"
            type="button"
            aria-label="Close AI chat"
            onClick={onClose}
          >
            <svg viewBox="0 0 16 16" width="14" height="14" fill="none" aria-hidden="true">
              <path d="M4 4 12 12M12 4 4 12" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
            </svg>
          </button>
        </header>

        <section className="ai-chat-preview">
          {loadingSession ? (
            <p>Preparing a frozen snapshot of the current notes...</p>
          ) : previewSummary ? (
            <>
              <p>{previewSummary}</p>
              {truncated ? (
                <div className="ai-chat-preview-meta">
                  <span>Oldest notes were trimmed to fit.</span>
                </div>
              ) : null}
            </>
          ) : (
            <p>Open a session to chat against the current notes.</p>
          )}
        </section>

        <section className="ai-chat-settings">
          <label className="field ai-chat-field">
            <span>Gemini API key</span>
            <input
              className="ai-chat-key-input"
              type="password"
              value={apiKey}
              placeholder="Paste your Gemini API key"
              autoComplete="off"
              spellCheck="false"
              onChange={(event) => onApiKeyChange(event.target.value)}
            />
          </label>
          <p className="ai-chat-hint">Stored locally on this device and sent only with AI chat requests.</p>
        </section>

        <div className="ai-chat-transcript" aria-live="polite">
          {messages.length === 0 ? (
            <div className="ai-chat-empty">
              <p>Ask a question about this snapshot.</p>
            </div>
          ) : (
            messages.map((message, index) => (
              <AiChatMessage
                key={`${message.role}-${index}`}
                message={message}
              />
            ))
          )}
        </div>

        <form className="ai-chat-composer" onSubmit={onSubmit}>
          <textarea
            className="ai-chat-input"
            rows={3}
            value={draft}
            placeholder="Ask about the current notes"
            onChange={(event) => onDraftChange(event.target.value)}
            disabled={loadingSession || sending}
          />
          <div className="ai-chat-composer-footer">
            {error ? <p className="ai-chat-error">{error}</p> : <span className="ai-chat-hint">This chat is not saved after close.</span>}
            <button
              className="primary-button"
              type="submit"
              disabled={loadingSession || sending || !draft.trim() || !apiKey.trim()}
            >
              {sending ? "Thinking..." : "Send"}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
