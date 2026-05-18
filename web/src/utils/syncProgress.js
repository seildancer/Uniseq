function normalizeSyncProgressToken(value) {
  if (typeof value !== "string") {
    return "";
  }

  return value
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/[\s-]+/g, "_")
    .replace(/_+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase();
}

function normalizeCount(value) {
  const count = Number(value);
  return Number.isFinite(count) && count >= 0 ? count : 0;
}

export function normalizeSyncProgressOperation(value) {
  return normalizeSyncProgressToken(value);
}

export function normalizeSyncProgressPhase(value) {
  return normalizeSyncProgressToken(value);
}

export function coerceSyncProgress(value) {
  if (!value || typeof value !== "object") {
    return null;
  }

  return {
    operation: normalizeSyncProgressOperation(value.operation),
    phase: normalizeSyncProgressPhase(value.phase),
    current: normalizeCount(value.current),
    total: normalizeCount(value.total),
    path: typeof value.path === "string" && value.path ? value.path : null,
    detail: typeof value.detail === "string" && value.detail ? value.detail : null,
  };
}
