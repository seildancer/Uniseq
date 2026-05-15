import { useEffect, useRef } from "react";
import { addDaysToDateName, compareDateNames } from "../utils/streamDates.js";

const WHEEL_PAGE_THRESHOLD_PX = 24;
const WHEEL_PAGE_COOLDOWN_MS = 420;
const TOUCH_PAGE_THRESHOLD_PX = 64;
const SCROLL_BOUNDARY_EPSILON_PX = 2;

function canPage(container, direction) {
  const maxScrollTop = Math.max(0, container.scrollHeight - container.clientHeight);
  if (maxScrollTop <= SCROLL_BOUNDARY_EPSILON_PX) {
    return true;
  }

  if (direction === "newer") {
    return container.scrollTop <= SCROLL_BOUNDARY_EPSILON_PX;
  }

  return maxScrollTop - container.scrollTop <= SCROLL_BOUNDARY_EPSILON_PX;
}

function nextDateName(selectedDate, latestDateName, direction) {
  const candidate = addDaysToDateName(selectedDate, direction === "newer" ? 1 : -1);
  if (
    direction === "newer"
    && latestDateName
    && compareDateNames(candidate, latestDateName) > 0
  ) {
    return null;
  }
  return candidate;
}

export function useMobileStreamDatePager({
  enabled,
  selectedDate,
  latestDateName,
  scrollContainerRef,
  onSelectDate,
}) {
  const navigationLockedRef = useRef(false);
  const lastWheelPageAtRef = useRef(0);
  const touchStateRef = useRef(null);

  useEffect(() => {
    navigationLockedRef.current = false;
    touchStateRef.current = null;
  }, [selectedDate]);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!enabled || !container || typeof onSelectDate !== "function") {
      return undefined;
    }

    function pageDate(direction, event, source) {
      if (navigationLockedRef.current || !canPage(container, direction)) {
        return false;
      }
      if (source === "wheel" && Date.now() - lastWheelPageAtRef.current < WHEEL_PAGE_COOLDOWN_MS) {
        return false;
      }

      const nextDate = nextDateName(selectedDate, latestDateName, direction);
      if (!nextDate) {
        return false;
      }

      navigationLockedRef.current = true;
      if (source === "wheel") {
        lastWheelPageAtRef.current = Date.now();
      }
      event?.preventDefault();
      onSelectDate(nextDate);
      return true;
    }

    function handleWheel(event) {
      const absY = Math.abs(event.deltaY);
      if (absY < WHEEL_PAGE_THRESHOLD_PX || absY <= Math.abs(event.deltaX)) {
        return;
      }

      pageDate(event.deltaY < 0 ? "newer" : "older", event, "wheel");
    }

    function handleTouchStart(event) {
      if (event.touches.length !== 1) {
        touchStateRef.current = null;
        return;
      }

      touchStateRef.current = {
        startY: event.touches[0].clientY,
        lastY: event.touches[0].clientY,
        direction: null,
        didPage: false,
      };
    }

    function handleTouchMove(event) {
      const touchState = touchStateRef.current;
      if (!touchState || touchState.didPage || event.touches.length !== 1) {
        return;
      }

      const currentY = event.touches[0].clientY;
      const previousY = touchState.lastY;
      const movementY = currentY - previousY;
      if (Math.abs(movementY) < 1) {
        return;
      }

      const direction = movementY > 0 ? "newer" : "older";
      if (!canPage(container, direction)) {
        touchState.startY = currentY;
        touchState.lastY = currentY;
        touchState.direction = null;
        return;
      }

      if (touchState.direction !== direction) {
        touchState.startY = previousY;
        touchState.direction = direction;
      }
      touchState.lastY = currentY;

      const boundaryDelta = direction === "older"
        ? touchState.startY - currentY
        : currentY - touchState.startY;
      if (boundaryDelta < TOUCH_PAGE_THRESHOLD_PX) {
        return;
      }

      const didPage = pageDate(direction, event, "touch");
      touchState.didPage = didPage;
    }

    function handleTouchEnd() {
      touchStateRef.current = null;
    }

    container.addEventListener("wheel", handleWheel, { passive: false, capture: true });
    container.addEventListener("touchstart", handleTouchStart, { passive: true, capture: true });
    container.addEventListener("touchmove", handleTouchMove, { passive: false, capture: true });
    container.addEventListener("touchend", handleTouchEnd, true);
    container.addEventListener("touchcancel", handleTouchEnd, true);

    return () => {
      container.removeEventListener("wheel", handleWheel, true);
      container.removeEventListener("touchstart", handleTouchStart, true);
      container.removeEventListener("touchmove", handleTouchMove, true);
      container.removeEventListener("touchend", handleTouchEnd, true);
      container.removeEventListener("touchcancel", handleTouchEnd, true);
    };
  }, [enabled, latestDateName, onSelectDate, scrollContainerRef, selectedDate]);
}
