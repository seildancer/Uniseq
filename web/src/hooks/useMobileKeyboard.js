import { useState, useEffect } from "react";

const MOBILE_QUERY = "(max-width: 820px), (pointer: coarse)";
const MIN_KEYBOARD_HEIGHT = 50;

function getKeyboardHeight() {
  const vv = window.visualViewport;
  if (!vv) return 0;
  return Math.max(0, Math.round(window.innerHeight - vv.offsetTop - vv.height));
}

function getVisibleViewportHeight() {
  const vv = window.visualViewport;
  if (!vv) return window.innerHeight;
  return Math.max(0, Math.round(vv.height + vv.offsetTop));
}

function isEditable(el) {
  if (!el) return false;
  const tag = el.tagName?.toLowerCase();
  return tag === "input" || tag === "textarea" || el.isContentEditable;
}

export function useMobileKeyboard() {
  const [isMobile, setIsMobile] = useState(
    () => window.matchMedia(MOBILE_QUERY).matches
  );
  const [keyboardHeight, setKeyboardHeight] = useState(0);
  const [visibleViewportHeight, setVisibleViewportHeight] = useState(() => getVisibleViewportHeight());
  const [hasFocus, setHasFocus] = useState(false);

  useEffect(() => {
    const mq = window.matchMedia(MOBILE_QUERY);
    const onMqChange = (e) => setIsMobile(e.matches);
    mq.addEventListener("change", onMqChange);
    return () => mq.removeEventListener("change", onMqChange);
  }, []);

  useEffect(() => {
    if (!isMobile) return;
    let rafId = null;
    let timer = null;

    function update() {
      cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(() => {
        setKeyboardHeight(getKeyboardHeight());
        setVisibleViewportHeight(getVisibleViewportHeight());
        setHasFocus(isEditable(document.activeElement));
      });
    }

    function debounced() {
      clearTimeout(timer);
      timer = setTimeout(update, 60);
    }

    const vv = window.visualViewport;
    vv?.addEventListener("resize", debounced);
    vv?.addEventListener("scroll", debounced);
    document.addEventListener("focusin", debounced);
    document.addEventListener("focusout", debounced);

    return () => {
      clearTimeout(timer);
      cancelAnimationFrame(rafId);
      vv?.removeEventListener("resize", debounced);
      vv?.removeEventListener("scroll", debounced);
      document.removeEventListener("focusin", debounced);
      document.removeEventListener("focusout", debounced);
    };
  }, [isMobile]);

  return {
    isMobile,
    isKeyboardVisible: isMobile && hasFocus && keyboardHeight > MIN_KEYBOARD_HEIGHT,
    keyboardHeight,
    visibleViewportHeight,
  };
}
