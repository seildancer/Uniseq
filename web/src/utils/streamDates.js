export function todayDateName() {
  const now = new Date();
  return `${String(now.getFullYear()).padStart(4, "0")}_${String(now.getMonth() + 1).padStart(2, "0")}_${String(now.getDate()).padStart(2, "0")}`;
}

const MONTH_NAMES_FULL = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];

export function formatDateLabel(dateName) {
  if (!dateName || dateName.length < 10) {
    return dateName;
  }
  const year = dateName.slice(0, 4);
  const month = Number.parseInt(dateName.slice(5, 7), 10);
  const day = Number.parseInt(dateName.slice(8, 10), 10);
  return `${MONTH_NAMES_FULL[month - 1]} ${day}, ${year}`;
}

export function yearFromDateName(dateName) {
  return Number.parseInt(dateName.slice(0, 4), 10);
}

export function monthFromDateName(dateName) {
  return Number.parseInt(dateName.slice(5, 7), 10) - 1;
}

function parseDateName(dateName) {
  const year = Number.parseInt(dateName.slice(0, 4), 10);
  const month = Number.parseInt(dateName.slice(5, 7), 10) - 1;
  const day = Number.parseInt(dateName.slice(8, 10), 10);
  return new Date(Date.UTC(year, month, day));
}

const MS_PER_DAY = 24 * 60 * 60 * 1000;

function formatDateName(date) {
  return `${String(date.getUTCFullYear()).padStart(4, "0")}_${String(date.getUTCMonth() + 1).padStart(2, "0")}_${String(date.getUTCDate()).padStart(2, "0")}`;
}

function daysBetweenDateNames(fromDateName, toDateName) {
  return Math.round((parseDateName(toDateName) - parseDateName(fromDateName)) / MS_PER_DAY);
}

export function addDaysToDateName(dateName, days) {
  const date = parseDateName(dateName);
  date.setUTCDate(date.getUTCDate() + days);
  return formatDateName(date);
}

export function compareDateNames(leftDateName, rightDateName) {
  if (leftDateName === rightDateName) {
    return 0;
  }
  return leftDateName < rightDateName ? -1 : 1;
}

export function buildDateRange(fromDateName, toDateName) {
  const dayDelta = daysBetweenDateNames(fromDateName, toDateName);
  const step = dayDelta < 0 ? -1 : 1;
  const count = Math.abs(dayDelta) + 1;
  return Array.from({ length: count }, (_, index) => addDaysToDateName(fromDateName, index * step));
}

export function buildCenteredDateRange(
  selectedDateName,
  count,
  earliestDateName = null,
  latestDateName = null,
) {
  const total = Math.max(1, count);
  let startOffset = -Math.floor((total - 1) / 2);
  let endOffset = total - 1 + startOffset;

  const earliestOffset = earliestDateName === null
    ? null
    : daysBetweenDateNames(selectedDateName, earliestDateName);
  const latestOffset = latestDateName === null
    ? null
    : daysBetweenDateNames(selectedDateName, latestDateName);

  if (earliestOffset !== null && startOffset < earliestOffset) {
    const shift = earliestOffset - startOffset;
    startOffset += shift;
    endOffset += shift;
  }

  if (latestOffset !== null && endOffset > latestOffset) {
    const shift = endOffset - latestOffset;
    startOffset -= shift;
    endOffset -= shift;
  }

  if (earliestOffset !== null) {
    startOffset = Math.max(startOffset, earliestOffset);
  }
  if (latestOffset !== null) {
    endOffset = Math.min(endOffset, latestOffset);
  }

  if (startOffset > endOffset) {
    return {
      startDateName: selectedDateName,
      endDateName: selectedDateName,
    };
  }

  return {
    startDateName: addDaysToDateName(selectedDateName, startOffset),
    endDateName: addDaysToDateName(selectedDateName, endOffset),
  };
}

export function maxDateName(dateNames, fallbackDateName) {
  let max = fallbackDateName;
  for (const dateName of dateNames) {
    if (!dateName) {
      continue;
    }
    if (!max || compareDateNames(dateName, max) > 0) {
      max = dateName;
    }
  }
  return max;
}

export function buildRecentStreamDateWindow(selectedDateName, count = 9) {
  const total = Math.max(1, count);
  return Array.from({ length: total }, (_, index) => addDaysToDateName(selectedDateName, -index));
}
