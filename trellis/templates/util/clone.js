// Lightweight deep clone helper for browser + server usage.
// Uses structuredClone when available, falls back to JSON clone for plain data.
export function clone(value) {
  if (typeof structuredClone === "function") {
    return structuredClone(value);
  }

  try {
    return JSON.parse(JSON.stringify(value));
  } catch (err) {
    console.warn("clone: fallback failed, returning original reference", err);
    return value;
  }
}
