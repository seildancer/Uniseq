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
