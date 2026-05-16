import { useEffect, useRef } from "react";

function dispatchKey(key, modifiers = {}) {
  const target = document.activeElement;
  if (!target) return;

  const init = {
    key,
    bubbles: true,
    cancelable: true,
    shiftKey: modifiers.shift ?? false,
    ctrlKey: modifiers.ctrl ?? false,
  };

  target.dispatchEvent(new KeyboardEvent("keydown", init));
  target.dispatchEvent(new KeyboardEvent("keyup", init));
}

function Btn({ icon, title, onPress }) {
  const ref = useRef(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const handleTouchStart = (event) => {
      event.preventDefault();
      onPress();
    };
    el.addEventListener("touchstart", handleTouchStart, { passive: false });
    return () => el.removeEventListener("touchstart", handleTouchStart);
  }, [onPress]);

  return (
    <button
      ref={ref}
      className="mobile-keyboard-bar-btn"
      title={title}
      onMouseDown={(event) => {
        event.preventDefault();
        onPress();
      }}
      type="button"
    >
      {icon}
    </button>
  );
}

const SHORTCUT_BUTTONS = [
  {
    title: "Shift+Tab (dedent)",
    icon: (
      <svg viewBox="0 0 20 14" width="20" height="14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M19 7H8M12 3 8 7l4 4" />
        <line x1="1" y1="2" x2="1" y2="12" strokeWidth="2" />
      </svg>
    ),
    onPress: () => dispatchKey("Tab", { shift: true }),
  },
  {
    title: "Tab (indent)",
    icon: (
      <svg viewBox="0 0 20 14" width="20" height="14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M1 7h11M8 3l4 4-4 4" />
        <line x1="19" y1="2" x2="19" y2="12" strokeWidth="2" />
      </svg>
    ),
    onPress: () => dispatchKey("Tab"),
  },
  {
    title: "Shift+Enter (new line)",
    icon: (
      <svg viewBox="0 0 16 14" width="16" height="14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M13 2v4a2 2 0 0 1-2 2H3" />
        <path d="M6 5 3 8l3 3" />
      </svg>
    ),
    onPress: () => dispatchKey("Enter", { shift: true }),
  },
  {
    title: "Undo",
    icon: (
      <svg viewBox="0 0 16 14" width="16" height="14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M2.5 5A5.5 5.5 0 1 1 4 9.5" />
        <path d="M2.5 1.5v3.5H6" />
      </svg>
    ),
    onPress: () => dispatchKey("z", { ctrl: true }),
  },
  {
    title: "Redo",
    icon: (
      <svg viewBox="0 0 16 14" width="16" height="14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M13.5 5A5.5 5.5 0 1 0 12 9.5" />
        <path d="M13.5 1.5v3.5H10" />
      </svg>
    ),
    onPress: () => dispatchKey("z", { ctrl: true, shift: true }),
  },
];

export function MobileKeyboardBar({ keyboardHeight }) {
  return (
    <div className="mobile-keyboard-bar" style={{ bottom: keyboardHeight }}>
      {SHORTCUT_BUTTONS.map((button) => (
        <Btn key={button.title} icon={button.icon} title={button.title} onPress={button.onPress} />
      ))}
    </div>
  );
}
