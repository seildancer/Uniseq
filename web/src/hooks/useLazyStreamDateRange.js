import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  addDaysToDateName,
  buildCenteredDateRange,
  buildDateRange,
  compareDateNames,
} from "../utils/streamDates.js";

const INITIAL_RENDER_DAYS = 21;
const EXPAND_DAYS = 14;
const SCROLL_THRESHOLD_PX = 480;

function rangeEquals(leftRange, rightRange) {
  return leftRange.startDateName === rightRange.startDateName
    && leftRange.endDateName === rightRange.endDateName;
}

function buildInitialRange(selectedDate, earliestDateName, latestDateName) {
  return buildCenteredDateRange(selectedDate, INITIAL_RENDER_DAYS, earliestDateName, latestDateName);
}

export function useLazyStreamDateRange({
  selectedDate,
  earliestDateName = null,
  latestDateName = null,
  scrollContainerRef,
  disabled = false,
}) {
  const [range, setRange] = useState(() => buildInitialRange(selectedDate, earliestDateName, latestDateName));
  const pendingTopExpansionRef = useRef(null);

  useEffect(() => {
    setRange((currentRange) => {
      const nextRange = buildInitialRange(selectedDate, earliestDateName, latestDateName);
      return rangeEquals(currentRange, nextRange) ? currentRange : nextRange;
    });
  }, [selectedDate, earliestDateName, latestDateName]);

  const canExpandOlder = !earliestDateName || compareDateNames(range.startDateName, earliestDateName) > 0;
  const canExpandNewer = !latestDateName || compareDateNames(range.endDateName, latestDateName) < 0;

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (disabled || !container) {
      return undefined;
    }

    function expandOlder() {
      if (!canExpandOlder) {
        return;
      }

      setRange((currentRange) => {
        let nextStartDateName = addDaysToDateName(currentRange.startDateName, -EXPAND_DAYS);
        if (earliestDateName && compareDateNames(nextStartDateName, earliestDateName) < 0) {
          nextStartDateName = earliestDateName;
        }

        const nextRange = {
          startDateName: nextStartDateName,
          endDateName: currentRange.endDateName,
        };
        if (rangeEquals(currentRange, nextRange)) {
          return currentRange;
        }
        return nextRange;
      });
    }

    function expandNewer() {
      if (!canExpandNewer) {
        return;
      }

      pendingTopExpansionRef.current = container.scrollHeight;
      setRange((currentRange) => {
        let nextEndDateName = addDaysToDateName(currentRange.endDateName, EXPAND_DAYS);
        if (latestDateName && compareDateNames(nextEndDateName, latestDateName) > 0) {
          nextEndDateName = latestDateName;
        }

        const nextRange = {
          startDateName: currentRange.startDateName,
          endDateName: nextEndDateName,
        };
        if (rangeEquals(currentRange, nextRange)) {
          pendingTopExpansionRef.current = null;
          return currentRange;
        }
        return nextRange;
      });
    }

    function handleScroll() {
      if (container.scrollTop <= SCROLL_THRESHOLD_PX && canExpandNewer) {
        expandNewer();
        return;
      }

      const remainingBottom = container.scrollHeight - container.scrollTop - container.clientHeight;
      if (remainingBottom <= SCROLL_THRESHOLD_PX && canExpandOlder) {
        expandOlder();
      }
    }

    handleScroll();
    container.addEventListener("scroll", handleScroll, { passive: true });
    return () => {
      container.removeEventListener("scroll", handleScroll);
    };
  }, [
    canExpandNewer,
    canExpandOlder,
    disabled,
    earliestDateName,
    latestDateName,
    range.endDateName,
    range.startDateName,
    scrollContainerRef,
  ]);

  useLayoutEffect(() => {
    const previousScrollHeight = pendingTopExpansionRef.current;
    if (previousScrollHeight === null) {
      return;
    }

    const container = scrollContainerRef.current;
    pendingTopExpansionRef.current = null;
    if (!container) {
      return;
    }

    const heightDelta = container.scrollHeight - previousScrollHeight;
    if (heightDelta > 0) {
      container.scrollTop += heightDelta;
    }
  }, [range, scrollContainerRef]);

  const visibleDates = useMemo(
    () => buildDateRange(range.endDateName, range.startDateName),
    [range.endDateName, range.startDateName],
  );

  return {
    visibleDates,
  };
}
