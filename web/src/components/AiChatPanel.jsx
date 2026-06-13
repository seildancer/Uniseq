import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import breaks from "remark-breaks";

function AiChatThinking() {
  return (
    <div className="ai-chat-thinking">
      <div className="ai-chat-message-label">AI</div>
      <div className="ai-chat-thinking-dots">
        <span /><span /><span />
      </div>
    </div>
  );
}

function AiChatMessage({ message }) {
  return (
    <article className={`ai-chat-message ai-chat-message--${message.role}`}>
      {message.role === "assistant" && (
        <div className="ai-chat-message-label">AI</div>
      )}
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
  model,
  models,
  loadingSession,
  sending,
  error,
  onClose,
  onDraftChange,
  onApiKeyChange,
  onModelChange,
  onSubmit,
}) {
  const transcriptRef = useRef(null);
  const textareaRef = useRef(null);
  const [keyExpanded, setKeyExpanded] = useState(!apiKey);

  useEffect(() => {
    const el = transcriptRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [messages, sending]);

  useEffect(() => {
    const ta = textareaRef.current;
    if (ta) {
      ta.style.height = "auto";
      ta.style.height = `${ta.scrollHeight}px`;
    }
  }, [draft]);

  function handleDraftChange(event) {
    onDraftChange(event.target.value);
  }

  function handleKeyDown(event) {
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
    event.preventDefault();
    onSubmit(event);
  }

  if (!isOpen) return null;

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
            <div className="ai-chat-header-title">
              <strong>AI Chat</strong>
              <span className="feature-badge feature-badge--soft">Beta</span>
            </div>
            {loadingSession && <span>Loading context...</span>}
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
        <section className="ai-chat-settings">
          {apiKey && !keyExpanded ? (
            <div className="ai-chat-key-compact">
              <span className="ai-chat-key-set">API key set</span>
              <button
                type="button"
                className="ai-chat-key-edit"
                onClick={() => setKeyExpanded(true)}
              >
                Edit
              </button>
            </div>
          ) : (
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
                onBlur={() => { if (apiKey) setKeyExpanded(false); }}
              />
            </label>
          )}
          <label className="field ai-chat-field">
            <span>Model</span>
            <select value={model} onChange={(event) => onModelChange(event.target.value)}>
              {models.map((entry) => (
                <option key={entry.value} value={entry.value}>
                  {entry.label} ({entry.status})
                </option>
              ))}
            </select>
          </label>
          <p className="ai-chat-hint">Stored locally and sent only with AI chat requests.</p>
        </section>

        {(loadingSession || previewSummary) && (
          <section className="ai-chat-preview">
            {loadingSession ? (
              <p>Preparing a snapshot of the current notes...</p>
            ) : (
              <>
                <p>{previewSummary}</p>
                {truncated && (
                  <div className="ai-chat-preview-meta">
                    <span>Oldest notes were trimmed to fit.</span>
                  </div>
                )}
              </>
            )}
          </section>
        )}

        <div className="ai-chat-transcript" ref={transcriptRef} aria-live="polite">
          {messages.length === 0 ? null : (
            messages.map((message, index) => (
              <AiChatMessage
                key={`${message.role}-${index}`}
                message={message}
              />
            ))
          )}
          {sending && <AiChatThinking />}
        </div>

        <form className="ai-chat-composer" onSubmit={onSubmit}>
          <textarea
            ref={textareaRef}
            className="ai-chat-input"
            rows={1}
            value={draft}
            placeholder="Ask about the current notes"
            onChange={handleDraftChange}
            onKeyDown={handleKeyDown}
            disabled={loadingSession || sending}
          />
          <div className="ai-chat-composer-footer">
            {error ? (
              <p className="ai-chat-error">{error}</p>
            ) : (
              <span className="ai-chat-hint">
                <kbd>Enter</kbd> to send, <kbd>Shift</kbd><kbd>Enter</kbd> for newline
              </span>
            )}
            <button
              className="primary-button"
              type="submit"
              disabled={loadingSession || sending || !draft.trim() || !apiKey.trim()}
            >
              Send
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
