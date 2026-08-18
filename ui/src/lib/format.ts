const numberFormat = new Intl.NumberFormat();

const timeFormat = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
});

const dateTimeFormat = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});

export function formatPoints(value: number): string {
  return numberFormat.format(value);
}

/** Signed points delta, e.g. "+1,240". */
export function formatDelta(value: number): string {
  const sign = value > 0 ? "+" : "";
  return `${sign}${numberFormat.format(value)}`;
}

export function formatTime(date: Date): string {
  return timeFormat.format(date);
}

export function formatDateTime(date: Date): string {
  return dateTimeFormat.format(date);
}

/** Seconds to "m:ss", used by the prediction countdown. */
export function formatCountdown(totalSeconds: number): string {
  const clamped = Math.max(0, Math.floor(totalSeconds));
  const minutes = Math.floor(clamped / 60);
  const seconds = clamped % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

export function formatPercent(value: number): string {
  return `${Math.round(value)}%`;
}
