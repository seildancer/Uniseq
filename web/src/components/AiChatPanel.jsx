import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import ReactMarkdown from "react-markdown";
import breaks from "remark-breaks";
import { aiChatScopeLabel, sameAiChatContext } from "../utils/aiChat.js";
import "./AiChatPanel.css";

const ICON_PATHS = {
  close: "m5 5 10 10M15 5 5 15",
  key: "M12 3a4 4 0 1 1-2.9 6.8L4 15.9V18H2v-4l6.9-6.9A4 4 0 0 1 12 3Z",
  plus: "M10 4v12M4 10h12",
  chevron: "m6 8 4 4 4-4",
  back: "m11 5-5 5 5 5M6 10h10",
  history: "M3 6v4h4M3.6 10a6.5 6.5 0 1 1 1.8 4.5M10 6v4l3 2",
  send: "M10 16V4M5 9l5-5 5 5",
  stop: "M6 6h8v8H6z",
  copy: "M7 7h9v9H7zM4 12V4h8",
  save: "M5 3h10v14l-5-3-5 3z",
  lock: "M6 8V6a4 4 0 0 1 8 0v2M5 8h10v9H5z",
  eye: "M2 10s3-5 8-5 8 5 8 5-3 5-8 5-8-5-8-5Zm8-2a2 2 0 1 0 0 4 2 2 0 0 0 0-4",
};

function Icon({ name }) {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 20 20"
      fill="none"
      aria-hidden="true"
    >
      {name === "more" ? (
        <g fill="currentColor">
          <circle cx="4" cy="10" r="1" />
          <circle cx="10" cy="10" r="1" />
          <circle cx="16" cy="10" r="1" />
        </g>
      ) : (
        <path
          d={ICON_PATHS[name]}
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      )}
    </svg>
  );
}

function IconButton({ icon, label, ...props }) {
  return (
    <button
      className="ai-icon-button"
      type="button"
      aria-label={label}
      title={label}
      {...props}
    >
      <Icon name={icon} />
    </button>
  );
}

function KeySetup({ apiKey, onSave, onOpenLink, onDone, disabled }) {
  const [replacing, setReplacing] = useState(false);
  const [value, setValue] = useState("");
  const [visible, setVisible] = useState(false);
  const [problem, setProblem] = useState("");
  const inputRef = useRef(null);
  const entering = !apiKey || replacing;

  useEffect(() => {
    if (entering) inputRef.current?.focus();
  }, [entering]);

  function save(event) {
    event.preventDefault();
    const key = value.trim();
    if (!key) {
      setProblem("Paste an API key to continue.");
      return;
    }
    onSave(key);
    setValue("");
    setReplacing(false);
    onDone();
  }

  return (
    <div className="ai-connection">
      {entering ? (
        <form className="ai-key-setup" onSubmit={save}>
          <div className="ai-connection-mark" aria-hidden="true">
            <Icon name="lock" />
          </div>
          <h2>{replacing ? "Replace your key" : "Connect OpenAI"}</h2>
          <p>
            {replacing
              ? "Your current key stays saved until you replace it."
              : "Add your API key to chat with your notes."}
          </p>
          <div className="ai-key-entry">
            <input
              ref={inputRef}
              type={visible ? "text" : "password"}
              value={value}
              aria-label="OpenAI API key"
              placeholder="Paste your API key"
              autoComplete="off"
              autoCapitalize="off"
              spellCheck={false}
              onChange={(event) => {
                setValue(event.target.value);
                setProblem("");
              }}
            />
            <IconButton
              icon="eye"
              label={visible ? "Hide key" : "Show key"}
              aria-pressed={visible}
              onClick={() => setVisible(!visible)}
            />
          </div>
          {problem && (
            <p className="ai-error" role="alert">
              {problem}
            </p>
          )}
          <button
            className="ai-solid-button"
            type="submit"
            disabled={!value.trim()}
          >
            {replacing ? "Replace key" : "Save key"}
          </button>
          {replacing ? (
            <button
              className="ai-text-button"
              type="button"
              onClick={() => {
                setReplacing(false);
                setValue("");
              }}
            >
              Cancel
            </button>
          ) : (
            <button
              className="ai-text-button"
              type="button"
              onClick={() => onOpenLink("https://platform.openai.com/api-keys")}
            >
              Get an API key <span aria-hidden="true">&#8599;</span>
            </button>
          )}
          <small>
            Your key is saved on this device. Selected notes and messages are
            sent to OpenAI when you chat.
          </small>
        </form>
      ) : (
        <div className="ai-key-saved">
          <div className="ai-connection-mark" aria-hidden="true">
            <Icon name="lock" />
          </div>
          <h2>OpenAI</h2>
          <p>
            <span className="ai-saved-dot" /> Key saved on this device
          </p>
          <div className="ai-key-mask" aria-label="API key is hidden">
            &#8226;&#8226;&#8226;&#8226; &#8226;&#8226;&#8226;&#8226;
            &#8226;&#8226;&#8226;&#8226;
          </div>
          <div className="ai-key-actions">
            <button
              className="ai-soft-button"
              type="button"
              disabled={disabled}
              onClick={() => setReplacing(true)}
            >
              Replace key
            </button>
            <button
              className="ai-text-button"
              type="button"
              disabled={disabled}
              onClick={() => onSave("")}
            >
              Remove
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

const ChatMessage = memo(function ChatMessage({ message, streaming, onSave }) {
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState("");
  const copyTimerRef = useRef(null);
  useEffect(() => () => clearTimeout(copyTimerRef.current), []);

  async function copy() {
    try {
      await navigator.clipboard.writeText(message.content);
      setCopied(true);
      setCopyError("");
      clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => setCopied(false), 1800);
    } catch {
      setCopyError("Could not copy. Select the text to copy it.");
    }
  }

  if (!message.content && !streaming) return null;
  return (
    <article className={`ai-message ai-message--${message.role}`}>
      <div className="ai-message-body">
        {message.content ? (
          <ReactMarkdown
            remarkPlugins={[breaks]}
            components={{
              a: ({ children, ...props }) => (
                <a {...props} target="_blank" rel="noopener noreferrer">
                  {children}
                </a>
              ),
            }}
          >
            {message.content}
          </ReactMarkdown>
        ) : (
          <span className="ai-thinking" aria-label="Preparing response">
            <i />
            <i />
            <i />
          </span>
        )}
        {streaming && message.content && (
          <span className="ai-stream-cursor" aria-hidden="true" />
        )}
      </div>
      {message.role === "assistant" && message.content && !streaming && (
        <div className="ai-message-actions">
          <IconButton
            icon="copy"
            label={copied ? "Copied" : "Copy answer"}
            onClick={copy}
          />
          <IconButton
            icon="save"
            label="Save to note"
            onClick={() => onSave(message.content)}
          />
          {copied && <span role="status">Copied</span>}
        </div>
      )}
      {copyError && (
        <p className="ai-error" role="status">
          {copyError}
        </p>
      )}
    </article>
  );
});

export default function AiChatPanel({
  isOpen,
  isMobile,
  isPrivate,
  sessions = [],
  activeSessionId,
  contextSpec,
  openingContextSpec,
  scopeOptions = [],
  previewSummary,
  truncated,
  messages,
  draft,
  apiKey,
  model,
  models,
  loadingSession,
  sending,
  stopping,
  requestReady,
  error,
  responseStatus,
  canRetry,
  viewportHeight,
  onClose,
  onNewChat,
  onNewPrivateChat,
  onSelectSession,
  onDeleteSession,
  onDraftChange,
  onApiKeyChange,
  onModelChange,
  onSubmit,
  onStop,
  onRetry,
  onSaveToNote,
  onOpenLink,
}) {
  const dialogRef = useRef(null);
  const transcriptRef = useRef(null);
  const textareaRef = useRef(null);
  const previousFocusRef = useRef(document.activeElement);
  const followRef = useRef(true);
  const [view, setView] = useState("chat");
  const [showLatest, setShowLatest] = useState(false);
  const [saveContent, setSaveContent] = useState("");
  const [saveTitle, setSaveTitle] = useState("AI notes");
  const [saveParent, setSaveParent] = useState("");
  const [savingNote, setSavingNote] = useState(false);
  const [saveError, setSaveError] = useState("");
  const newThread = messages.length === 0;
  const needsSetup = view === "chat" && !apiKey && newThread;
  const compact = view === "chat" && newThread && apiKey;
  const busy = sending || loadingSession;
  const scopeLabel = aiChatScopeLabel(contextSpec);
  const contextKey = JSON.stringify(contextSpec);
  const choices = scopeOptions.some((option) =>
    sameAiChatContext(option.spec, contextSpec),
  )
    ? scopeOptions
    : [{ label: scopeLabel, spec: contextSpec }, ...scopeOptions].filter(
        (option) => option.spec,
      );
  const selectedScope = JSON.stringify(
    choices.find((option) => sameAiChatContext(option.spec, contextSpec))
      ?.spec ?? contextSpec,
  );
  const scopeHelp =
    contextSpec?.kind === "page"
      ? "Includes subtopics and related notes. Fixed after sending."
      : "Includes notes across dates. Fixed after sending.";

  const previewSave = useCallback((content) => {
    setSaveContent(content);
    setSaveError("");
    setView("save");
  }, []);
  function returnToChat() {
    setView("chat");
  }
  function newChat(
    privateMode = isPrivate,
    spec = openingContextSpec ?? contextSpec,
    preserveDraft = false,
  ) {
    if (privateMode) onNewPrivateChat(spec, preserveDraft);
    else onNewChat(spec, preserveDraft);
    returnToChat();
  }
  function jumpToLatest() {
    followRef.current = true;
    transcriptRef.current?.scrollTo({
      top: transcriptRef.current.scrollHeight,
      behavior: "smooth",
    });
    setShowLatest(false);
  }
  async function saveNote(event) {
    event.preventDefault();
    setSavingNote(true);
    setSaveError("");
    try {
      await onSaveToNote({
        content: saveContent,
        title: saveTitle,
        parentPageId: saveParent,
      });
      returnToChat();
    } catch (problem) {
      setSaveError(problem.message);
    } finally {
      setSavingNote(false);
    }
  }

  useLayoutEffect(() => {
    followRef.current = true;
  }, [contextKey, messages[0]?.content]);
  useLayoutEffect(() => {
    const el = transcriptRef.current;
    if (!el) return;
    if (followRef.current) {
      el.scrollTop = el.scrollHeight;
      setShowLatest(false);
    } else setShowLatest(true);
  }, [messages, sending, activeSessionId, view]);
  useLayoutEffect(() => {
    const el = textareaRef.current;
    if (el) {
      el.style.height = "auto";
      el.style.height = `${Math.min(el.scrollHeight, 144)}px`;
    }
  }, [draft, view, needsSetup, newThread]);
  useEffect(() => {
    if (view === "chat" && apiKey && !isMobile && !loadingSession)
      textareaRef.current?.focus();
  }, [view, needsSetup, activeSessionId, isMobile, loadingSession]);
  useEffect(() => {
    const previous = previousFocusRef.current;
    if (!dialogRef.current?.contains(document.activeElement))
      dialogRef.current?.focus();
    return () => {
      if (previous?.isConnected) previous.focus();
    };
  }, []);
  useEffect(() => {
    function keyDown(event) {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        if (view !== "chat") returnToChat();
        else onClose();
      }
      if (event.key === "Tab") {
        const nodes = Array.from(
          dialogRef.current?.querySelectorAll(
            "button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href]",
          ) ?? [],
        ).filter((node) => node.getClientRects().length > 0);
        if (!nodes.length) return;
        const first = nodes[0],
          last = nodes[nodes.length - 1];
        if (
          event.shiftKey &&
          (document.activeElement === first ||
            !dialogRef.current.contains(document.activeElement))
        ) {
          event.preventDefault();
          last.focus();
        } else if (
          !event.shiftKey &&
          (document.activeElement === last ||
            !dialogRef.current.contains(document.activeElement))
        ) {
          event.preventDefault();
          first.focus();
        }
      }
    }
    document.addEventListener("keydown", keyDown, true);
    return () => document.removeEventListener("keydown", keyDown, true);
  }, [view, onClose]);

  if (!isOpen) return null;
  return (
    <div
      className="ai-chat-overlay"
      data-no-window-drag="true"
      style={{
        "--ai-chat-visible-height": viewportHeight
          ? `${viewportHeight}px`
          : "100dvh",
      }}
      onPointerDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        className={`ai-chat-dialog${compact ? " ai-chat-dialog--compose" : ""}${needsSetup || view === "connection" ? " ai-chat-dialog--connection" : ""}`}
        ref={dialogRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-label="AI chat"
      >
        <header className="ai-chat-header">
          <div className="ai-header-title">
            {view !== "chat" && (
              <IconButton
                icon="back"
                label="Back to chat"
                onClick={returnToChat}
              />
            )}
            <div>
              <strong>
                {view === "history"
                  ? "Conversations"
                  : view === "connection"
                    ? "Connection"
                    : view === "save"
                      ? "Save to note"
                      : "AI Chat"}
              </strong>
              {view === "chat" && !newThread && (
                <div
                  className="ai-fixed-context"
                  title={previewSummary}
                  aria-label={`Fixed context: ${scopeLabel}`}
                >
                  <Icon name="lock" />
                  <span>{scopeLabel}</span>
                  {isPrivate && (
                    <span className="ai-private-label">Private</span>
                  )}
                </div>
              )}
            </div>
          </div>
          <nav className="ai-chat-toolbar" aria-label="Chat controls">
            {view === "chat" && (
              <>
                {!newThread && (
                  <IconButton
                    icon="plus"
                    label="New chat"
                    disabled={busy}
                    onClick={() => newChat()}
                  />
                )}
                {(!needsSetup || sessions.length > 0) && (
                  <IconButton
                    icon="history"
                    label="Chat history"
                    onClick={() => setView("history")}
                  />
                )}
                {!needsSetup && (
                  <IconButton
                    icon="key"
                    label="Manage API key"
                    onClick={() => setView("connection")}
                  />
                )}
              </>
            )}
            <IconButton icon="close" label="Close AI chat" onClick={onClose} />
          </nav>
        </header>

        {needsSetup || view === "connection" ? (
          <KeySetup
            apiKey={apiKey}
            disabled={sending}
            onSave={onApiKeyChange}
            onDone={returnToChat}
            onOpenLink={onOpenLink}
          />
        ) : view === "history" ? (
          <div className="ai-history">
            {!sessions.length && (
              <div className="ai-view-empty">
                <p>No saved conversations yet.</p>
              </div>
            )}
            {sessions.map((session) => (
              <div
                key={session.session_id}
                className={`ai-session${session.session_id === activeSessionId ? " ai-session--active" : ""}`}
              >
                <button
                  className="ai-session-main"
                  type="button"
                  disabled={busy}
                  onClick={() => {
                    onSelectSession(session.session_id);
                    returnToChat();
                  }}
                >
                  <span>{session.title || "Untitled chat"}</span>
                  <small>{session.preview_summary}</small>
                </button>
                <IconButton
                  icon="close"
                  label={`Delete ${session.title || "chat"}`}
                  disabled={busy}
                  onClick={() => onDeleteSession(session.session_id)}
                />
              </div>
            ))}
          </div>
        ) : view === "save" ? (
          <form className="ai-save-form" onSubmit={saveNote}>
            <label htmlFor="ai-note-title">Note title</label>
            <input
              id="ai-note-title"
              value={saveTitle}
              disabled={savingNote}
              onChange={(event) => setSaveTitle(event.target.value)}
            />
            <label htmlFor="ai-note-parent">Save under</label>
            <select
              id="ai-note-parent"
              value={saveParent}
              disabled={savingNote}
              onChange={(event) => setSaveParent(event.target.value)}
            >
              <option value="">Top level</option>
              {scopeOptions
                .filter((option) => option.spec.kind === "page")
                .map((option) => (
                  <option key={option.spec.page_id} value={option.spec.page_id}>
                    {option.label}
                  </option>
                ))}
            </select>
            <div className="ai-save-preview">
              <ReactMarkdown remarkPlugins={[breaks]}>
                {saveContent}
              </ReactMarkdown>
            </div>
            {saveError && (
              <p className="ai-error" role="alert">
                {saveError}
              </p>
            )}
            <button
              className="ai-solid-button"
              type="submit"
              disabled={
                savingNote ||
                !saveTitle.trim() ||
                /[\\/:*?"<>|]/.test(saveTitle)
              }
            >
              {savingNote ? "Saving..." : "Save note"}
            </button>
          </form>
        ) : (
          <>
            {!newThread && (
              <div
                className="ai-transcript"
                ref={transcriptRef}
                aria-label="Conversation"
                aria-busy={busy}
                onScroll={() => {
                  const el = transcriptRef.current;
                  followRef.current =
                    el.scrollHeight - el.scrollTop - el.clientHeight < 64;
                  setShowLatest(!followRef.current);
                }}
              >
                {messages.map((message, index) => (
                  <ChatMessage
                    key={`${message.role}-${index}`}
                    message={message}
                    streaming={
                      sending &&
                      index === messages.length - 1 &&
                      message.role === "assistant"
                    }
                    onSave={previewSave}
                  />
                ))}
              </div>
            )}
            {!newThread && showLatest && (
              <button
                className="ai-latest"
                type="button"
                onClick={jumpToLatest}
              >
                &darr; Latest
              </button>
            )}
            <div className="ai-status" role="status" aria-live="polite">
              <span className="ai-sr-only">
                {loadingSession
                  ? "Preparing context"
                  : sending
                    ? stopping
                      ? "Stopping response"
                      : "Generating response"
                    : responseStatus}
              </span>
            </div>
            {apiKey ? (
              <div className="ai-compose-area">
                <form
                  className="ai-composer"
                  onSubmit={(event) => {
                    followRef.current = true;
                    onSubmit(event);
                  }}
                >
                  {newThread && (
                    <div className="ai-compose-context">
                      <label htmlFor="ai-chat-context">Chat about</label>
                      <select
                        id="ai-chat-context"
                        value={selectedScope || ""}
                        disabled={busy}
                        aria-describedby="ai-chat-context-help"
                        onChange={(event) =>
                          newChat(
                            isPrivate,
                            JSON.parse(event.target.value),
                            true,
                          )
                        }
                      >
                        {choices.map((option) => (
                          <option
                            key={JSON.stringify(option.spec)}
                            value={JSON.stringify(option.spec)}
                          >
                            {option.label}
                            {sameAiChatContext(option.spec, openingContextSpec)
                              ? " (current view)"
                              : ""}
                          </option>
                        ))}
                      </select>
                      <small id="ai-chat-context-help">
                        {loadingSession
                          ? "Preparing context..."
                          : truncated
                            ? "Some notes were trimmed to fit. Fixed after sending."
                            : scopeHelp}
                      </small>
                    </div>
                  )}
                  <textarea
                    className="ai-input"
                    ref={textareaRef}
                    rows={newThread ? 3 : 1}
                    value={draft}
                    aria-label="Message AI"
                    placeholder={
                      sending
                        ? "Draft your next question..."
                        : newThread
                          ? "Write your first message..."
                          : "Message..."
                    }
                    disabled={loadingSession}
                    onChange={(event) => onDraftChange(event.target.value)}
                    onKeyDown={(event) => {
                      if (
                        event.key === "Enter" &&
                        !event.shiftKey &&
                        !event.nativeEvent.isComposing &&
                        !isMobile
                      ) {
                        event.preventDefault();
                        if (!sending) {
                          followRef.current = true;
                          onSubmit(event);
                        }
                      }
                    }}
                  />
                  <div className="ai-composer-controls">
                    <div className="ai-composer-options">
                      <select
                        className="ai-model-select"
                        aria-label="AI model"
                        value={model}
                        disabled={sending}
                        onChange={(event) => onModelChange(event.target.value)}
                      >
                        {models.map((entry) => (
                          <option key={entry.value} value={entry.value}>
                            {entry.label}
                          </option>
                        ))}
                      </select>
                      {newThread && (
                        <button
                          className="ai-private-toggle"
                          type="button"
                          disabled={busy}
                          aria-pressed={isPrivate}
                          title="Private chats stay in memory and exclude workspace memory"
                          onClick={() => newChat(!isPrivate, contextSpec, true)}
                        >
                          <Icon name="lock" />
                          Private
                        </button>
                      )}
                    </div>
                    {sending ? (
                      <button
                        className="ai-send"
                        type="button"
                        aria-label={
                          stopping ? "Stopping response" : "Stop response"
                        }
                        disabled={stopping || !requestReady}
                        onClick={(event) => {
                          event.preventDefault();
                          onStop();
                        }}
                      >
                        <Icon name="stop" />
                        Stop
                      </button>
                    ) : (
                      <button
                        className="ai-send"
                        type="submit"
                        aria-label="Send message"
                        disabled={loadingSession || !draft.trim()}
                      >
                        <Icon name="send" />
                        Send
                      </button>
                    )}
                  </div>
                </form>
              </div>
            ) : (
              <button
                className="ai-connect-prompt"
                type="button"
                onClick={() => setView("connection")}
              >
                Add an API key to reply
              </button>
            )}
            {error ? (
              <div className="ai-feedback ai-error" role="alert">
                <span>{error}</span>
                {canRetry && (
                  <button
                    className="ai-text-button"
                    type="button"
                    disabled={busy}
                    onClick={onRetry}
                  >
                    Retry
                  </button>
                )}
              </div>
            ) : (
              responseStatus && <p className="ai-feedback">{responseStatus}</p>
            )}
          </>
        )}
      </section>
    </div>
  );
}
