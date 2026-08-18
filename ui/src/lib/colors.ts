/**
 * Deterministic per-channel colours, so a streamer keeps the same colour across
 * views and reloads. Hashing the id beats an insertion-order map, which shifts
 * every time the streamer list changes.
 */

/**
 * The five system hues lead, because most setups track only a handful of
 * channels. The rest are tuned neighbours rather than more of the same family:
 * up to ten lines can share one chart, and telling them apart is the job.
 */
const PALETTE = [
  "#d9a13b", // brass
  "#3d7dff", // team blue
  "#ff2f92", // team pink
  "#35c39a", // moss
  "#e2593f", // rust
  "#8f7ff5", // periwinkle
  "#4fb8e0", // cyan
  "#c2b23f", // olive
  "#e08bd0", // orchid
  "#5fa87a", // sage
];

/** Prediction outcomes in Twitch's own order: outcome 1 blue, outcome 2 pink. */
const OUTCOMES = ["#3d7dff", "#ff2f92", "#d9a13b", "#35c39a", "#8f7ff5", "#e2593f"];

export function channelColor(channelId: number): string {
  // Multiply to spread small consecutive ids across the palette.
  const index = Math.abs(channelId * 2654435761) % PALETTE.length;
  return PALETTE[index];
}

/**
 * Colour for a prediction outcome. The API's `Outcome` carries no colour field,
 * so its position in the list is the only signal - which matches how Twitch
 * itself colours them.
 */
export function outcomeColor(index: number): string {
  return OUTCOMES[index % OUTCOMES.length];
}

/** `#rrggbb` plus an alpha, in the `rgba()` form ECharts colour stops want. */
function withAlpha(hex: string, alpha: number): string {
  const value = hex.replace("#", "");
  const r = parseInt(value.slice(0, 2), 16);
  const g = parseInt(value.slice(2, 4), 16);
  const b = parseInt(value.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * Vertical fade for a chart's area fill, so the mass under the line reads as
 * accumulated value. Returned as a plain object rather than an
 * `echarts.graphic.LinearGradient`, which keeps the option serialisable and
 * avoids pulling more of echarts into the bundle.
 */
export function areaGradient(color: string) {
  return {
    type: "linear" as const,
    x: 0,
    y: 0,
    x2: 0,
    y2: 1,
    colorStops: [
      { offset: 0, color: withAlpha(color, 0.46) },
      { offset: 0.62, color: withAlpha(color, 0.12) },
      { offset: 1, color: withAlpha(color, 0) },
    ],
  };
}

export const chartPalette = PALETTE;
