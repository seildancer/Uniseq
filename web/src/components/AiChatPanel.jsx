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

function PlusIcon() {
  return (
    <svg viewBox="0 0 16 16" width="14" height="14" fill="none" aria-hidden="true">
      <path d="M8 3.25v9.5M3.25 8h9.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

function GhostIcon() {
  return (
    <svg viewBox="0 0 24 24" width="22" height="22" fill="none" aria-hidden="true">
      <path
        d="M12 2.75a6.75 6.75 0 0 0-6.75 6.75v10.24c0 .44.5.7.86.45L8.9 18.3l2.6 1.88c.3.21.7.21 1 0l2.6-1.88 2.8 1.89c.36.25.85-.01.85-.45V9.5A6.75 6.75 0 0 0 12 2.75Zm-2.45 7.1a1.05 1.05 0 1 1 0 2.1 1.05 1.05 0 0 1 0-2.1Zm4.9 5.48c-.61.53-1.43.8-2.45.8s-1.84-.27-2.45-.8a.78.78 0 1 1 1.03-1.16c.27.23.74.4 1.42.4s1.15-.17 1.42-.4a.78.78 0 1 1 1.03 1.16Zm0-3.38a1.05 1.05 0 1 1 0-2.1 1.05 1.05 0 0 1 0 2.1Z"
        fill="currentColor"
      />
    </svg>
  );
}

export default function AiChatPanel({
  isOpen,
  sessionTitle,
  isPrivate,
  sessions,
  activeSessionId,
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
  viewportHeight,
  keyboardHeight,
  keyboardVisible,
  onClose,
  onNewChat,
  onNewPrivateChat,
  onSelectSession,
  onDeleteSession,
  onDraftChange,
  onApiKeyChange,
  onModelChange,
  onSubmit,
}) {
  const transcriptRef = useRef(null);
  const textareaRef = useRef(null);
  const settingsRef = useRef(null);
  const [settingsExpanded, setSettingsExpanded] = useState(!apiKey);

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

  function handleSettingsBlur(event) {
    if (!apiKey) return;
    const nextFocus = event.relatedTarget;
    if (nextFocus && settingsRef.current?.contains(nextFocus)) return;
    setSettingsExpanded(false);
  }

  if (!isOpen) return null;

  const selectedModel = models.find((entry) => entry.value === model);
  const selectedModelLabel = selectedModel?.label ?? model;
  const overlayStyle = {
    "--ai-chat-visible-height": viewportHeight ? `${viewportHeight}px` : "100dvh",
    "--ai-chat-keyboard-height": `${keyboardHeight ?? 0}px`,
    "--ai-chat-keyboard-active": keyboardVisible ? "1" : "0",
  };

  return (
    <div
      className="ai-chat-overlay"
      data-no-window-drag="true"
      style={overlayStyle}
      onClick={onClose}
    >
      <section
        className="ai-chat-panel"
        role="dialog"
        aria-modal="true"
        aria-label="AI Chat"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="ai-chat-header">
            <div className="ai-chat-header-copy">
              <div className="ai-chat-header-title">
                <strong>AI Chat</strong>
                <span className="feature-badge feature-badge--soft">Beta</span>
                {isPrivate ? (
                  <span className="ai-chat-mode-badge">Private</span>
                ) : null}
              </div>
              {sessionTitle ? <span>{sessionTitle}</span> : null}
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
        <div className="ai-chat-layout">
          <aside className="ai-chat-sidebar">
            <section
              className="ai-chat-settings"
              ref={settingsRef}
              onBlur={handleSettingsBlur}
            >
              {apiKey && !settingsExpanded ? (
                <div className="ai-chat-settings-compact">
                  <div className="ai-chat-settings-summary">
                    <span>API key set</span>
                    <small>{selectedModelLabel}</small>
                  </div>
                  <button
                    type="button"
                    className="ai-chat-key-edit"
                    onClick={() => setSettingsExpanded(true)}
                  >
                    Edit
                  </button>
                </div>
              ) : (
                <>
                  {apiKey ? (
                    <div className="ai-chat-settings-compact">
                      <div className="ai-chat-settings-summary">
                        <span>AI settings</span>
                        <small>{selectedModelLabel}</small>
                      </div>
                      <button
                        type="button"
                        className="ai-chat-key-edit"
                        onClick={() => setSettingsExpanded(false)}
                      >
                        Done
                      </button>
                    </div>
                  ) : null}
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
                </>
              )}
              {settingsExpanded ? (
                <p className="ai-chat-hint">
                  {isPrivate
                    ? "Private chats stay in memory only. They are not saved to history or workspace memory."
                    : "Saved chats are stored locally. Deleting a chat removes the transcript only; workspace memory is kept."}
                </p>
              ) : null}
            </section>
            <div className="ai-chat-actions">
              <button
                type="button"
                className="ai-chat-action-button"
                onClick={onNewChat}
                disabled={loadingSession || sending}
              >
                <PlusIcon />
                <span>New chat</span>
              </button>
              <button
                type="button"
                className="ai-chat-action-button ai-chat-action-button--private"
                aria-label="Start private chat"
                title="Private chat"
                onClick={onNewPrivateChat}
                disabled={loadingSession || sending}
              >
                <GhostIcon />
              </button>
            </div>

            {Array.isArray(sessions) && sessions.length > 0 ? (
              <section className="ai-chat-sessions" aria-label="AI chat sessions">
                {sessions.slice(0, 24).map((session) => (
                  <div
                    key={session.session_id}
                    className={`ai-chat-session${session.session_id === activeSessionId ? " ai-chat-session--active" : ""}`}
                  >
                    <button
                      type="button"
                      className="ai-chat-session-main"
                      onClick={() => onSelectSession(session.session_id)}
                      disabled={loadingSession || sending}
                      title={session.preview_summary}
                    >
                      <span>{session.title || "New chat"}</span>
                      <small>{session.message_count ?? 0} msg</small>
                    </button>
                    <button
                      type="button"
                      className="ai-chat-session-delete"
                      aria-label={`Delete ${session.title || "chat"}`}
                      title="Delete chat"
                      onClick={() => onDeleteSession(session.session_id)}
                      disabled={loadingSession || sending}
                    >
                      <svg viewBox="0 0 16 16" width="14" height="14" fill="none" aria-hidden="true">
                        <path d="M5.25 5.25 10.75 10.75M10.75 5.25 5.25 10.75" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
                      </svg>
                    </button>
                  </div>
                ))}
              </section>
            ) : null}
          </aside>

          <main className="ai-chat-main">
            {previewSummary && (
              <section className="ai-chat-preview">
                <p>{previewSummary}</p>
                {truncated && (
                  <div className="ai-chat-preview-meta">
                    <span>Oldest notes were trimmed to fit.</span>
                  </div>
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
              <div className="ai-chat-composer-row">
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
                <button
                  className="primary-button ai-chat-send-button"
                  type="submit"
                  disabled={loadingSession || sending || !draft.trim() || !apiKey.trim()}
                >
                  Send
                </button>
              </div>
              {error ? <p className="ai-chat-error">{error}</p> : null}
            </form>
          </main>
        </div>
      </section>
    </div>
  );
}
