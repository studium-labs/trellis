// Lightweight deep clone helper for browser + server usage.
// Uses structuredClone when available, falls back to JSON clone for plain data.
export function clone<T>(value: T): T {
  if (typeof structuredClone === "function") {
    return structuredClone(value) as T;
  }

  try {
    return JSON.parse(JSON.stringify(value)) as T;
  } catch (err) {
    console.warn("clone: fallback failed, returning original reference", err);
    return value;
  }
}
