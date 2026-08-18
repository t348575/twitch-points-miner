/**
 * Time helpers.
 *
 * The analytics database stores `chrono::NaiveDateTime`, which serde renders
 * without an offset and with up to 9 fractional digits, e.g.
 * "2026-08-17T22:13:04.123456789". `new Date()` on that string is engine
 * dependent, so parse it explicitly.
 *
 * Requests go the other way: `POST /api/analytics/timeline` parses `from`/`to`
 * as RFC3339 *with* an offset and then converts to the server's local wall
 * clock (app/src/web_api/analytics.rs). So sending an absolute instant is
 * correct as long as the browser and the server share a timezone.
 */

const NAIVE_RE = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d+))?$/;

/** Parse an offset-less server timestamp as local time. */
export function parseNaiveLocal(value: string): Date {
  const m = NAIVE_RE.exec(value);
  if (!m) {
    // Already carries an offset, or an unexpected shape.
    return new Date(value);
  }
  const ms = m[7] ? Number(m[7].slice(0, 3).padEnd(3, "0")) : 0;
  return new Date(
    Number(m[1]),
    Number(m[2]) - 1,
    Number(m[3]),
    Number(m[4]),
    Number(m[5]),
    Number(m[6]),
    ms,
  );
}

/** Format a date for the timeline request body. */
export function toRfc3339(date: Date): string {
  return date.toISOString();
}

export function startOfToday(): Date {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d;
}

export function endOfToday(): Date {
  const d = startOfToday();
  d.setDate(d.getDate() + 1);
  return d;
}

const DAY_MS = 24 * 60 * 60 * 1000;

export function daysAgo(days: number, from: Date = startOfToday()): Date {
  return new Date(from.getTime() - days * DAY_MS);
}

/**
 * Local midnight on 1 January of the current year. Built from calendar parts
 * rather than subtracting days, so it stays correct across a DST boundary.
 */
export function startOfYear(): Date {
  return new Date(new Date().getFullYear(), 0, 1);
}

export function hoursAgo(hours: number, from: Date = new Date()): Date {
  return new Date(from.getTime() - hours * 60 * 60 * 1000);
}
