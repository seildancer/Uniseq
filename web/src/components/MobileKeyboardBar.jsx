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

function Btn({ label, title, onPress }) {
  return (
    <button
      className="mobile-keyboard-bar-btn"
      title={title}
      onMouseDown={(event) => {
        event.preventDefault();
        onPress();
      }}
      onTouchStart={(event) => {
        event.preventDefault();
        onPress();
      }}
      type="button"
    >
      {label}
    </button>
  );
}

const SHORTCUT_BUTTONS = [
  { label: "Tab", title: "Tab (indent)", onPress: () => dispatchKey("Tab") },
  { label: "Out", title: "Shift+Tab (dedent)", onPress: () => dispatchKey("Tab", { shift: true }) },
  { label: "Line", title: "Shift+Enter (new line)", onPress: () => dispatchKey("Enter", { shift: true }) },
  { label: "Undo", title: "Undo", onPress: () => dispatchKey("z", { ctrl: true }) },
  { label: "Redo", title: "Redo", onPress: () => dispatchKey("z", { ctrl: true, shift: true }) },
];

export function MobileKeyboardBar({ keyboardHeight }) {
  return (
    <div className="mobile-keyboard-bar" style={{ bottom: keyboardHeight }}>
      {SHORTCUT_BUTTONS.map((button) => (
        <Btn key={button.title} label={button.label} title={button.title} onPress={button.onPress} />
      ))}
    </div>
  );
}
