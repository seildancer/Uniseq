export function findTouchByIdentifier(touchList, identifier) {
  if (!touchList || identifier == null) {
    return null;
  }
  for (const touch of touchList) {
    if (touch.identifier === identifier) {
      return touch;
    }
  }
  return null;
}

export function startLongPressTouchDrag({
  event,
  longPressMs,
  setDragState,
  buildDragState,
  matchesDragState,
}) {
  // Touch drag in a scrollable list has to start on the browser's native pan
  // path. We only promote to drag after the long press has clearly won.
  if (event.touches.length !== 1) {
    return null;
  }

  const touch = event.touches[0];
  const nextDragState = buildDragState(touch);

  const timerId = window.setTimeout(() => {
    setDragState((current) => (
      matchesDragState(current, nextDragState)
        ? { ...current, active: true }
        : current
    ));
  }, longPressMs);

  setDragState(nextDragState);
  return timerId;
}

export function attachTouchDragListeners({
  dragState,
  setDragState,
  clearPendingDragState,
  moveSlopPx,
  updateHover,
  onDrop,
}) {
  const handleTouchMove = (event) => {
    const touch =
      findTouchByIdentifier(event.touches, dragState.pointerId)
      ?? findTouchByIdentifier(event.changedTouches, dragState.pointerId);
    if (!touch) {
      return;
    }

    if (!dragState.active) {
      const distance = Math.hypot(touch.clientX - dragState.startX, touch.clientY - dragState.startY);
      if (distance > moveSlopPx) {
        clearPendingDragState();
        setDragState(null);
      }
      return;
    }

    if (event.cancelable) {
      event.preventDefault();
    }

    updateHover(touch.clientX, touch.clientY);
  };

  const finishTouchDrag = async (event) => {
    const touch =
      findTouchByIdentifier(event.changedTouches, dragState.pointerId)
      ?? findTouchByIdentifier(event.touches, dragState.pointerId);
    if (!touch && event.type !== "touchcancel") {
      return;
    }

    clearPendingDragState();
    const currentDragState = dragState;
    setDragState(null);
    if (currentDragState.active) {
      await onDrop(currentDragState);
    }
  };

  window.addEventListener("touchmove", handleTouchMove, { passive: false });
  window.addEventListener("touchend", finishTouchDrag);
  window.addEventListener("touchcancel", finishTouchDrag);

  return () => {
    window.removeEventListener("touchmove", handleTouchMove);
    window.removeEventListener("touchend", finishTouchDrag);
    window.removeEventListener("touchcancel", finishTouchDrag);
  };
}
