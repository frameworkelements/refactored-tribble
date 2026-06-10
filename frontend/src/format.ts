/// Shared date/time formatting helpers for the UI.

export function formatDay(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

export function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatTimeRange(startIso: string, endIso: string): string {
  return `${formatTime(startIso)} – ${formatTime(endIso)}`;
}

/// Group items by their calendar day (derived from `iso`), preserving order.
export function groupByDay<T>(
  items: T[],
  iso: (item: T) => string
): { day: string; key: string; items: T[] }[] {
  const groups: { day: string; key: string; items: T[] }[] = [];
  const index = new Map<string, number>();
  for (const item of items) {
    const date = new Date(iso(item));
    const key = date.toISOString().slice(0, 10);
    if (!index.has(key)) {
      index.set(key, groups.length);
      groups.push({ day: formatDay(iso(item)), key, items: [] });
    }
    groups[index.get(key)!].items.push(item);
  }
  return groups;
}
