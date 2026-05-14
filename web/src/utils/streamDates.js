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

function formatDateName(date) {
  return `${String(date.getUTCFullYear()).padStart(4, "0")}_${String(date.getUTCMonth() + 1).padStart(2, "0")}_${String(date.getUTCDate()).padStart(2, "0")}`;
}

export function addDaysToDateName(dateName, days) {
  const date = parseDateName(dateName);
  date.setUTCDate(date.getUTCDate() + days);
  return formatDateName(date);
}

export function buildRecentStreamDateWindow(selectedDateName, count = 9) {
  const total = Math.max(1, count);
  return Array.from({ length: total }, (_, index) => addDaysToDateName(selectedDateName, -index));
}
