/**
 * Percentage scaling boundary.
 *
 * The backend divides percentages by 100 when it loads the config
 * (`Normalize` in common/src/config/strategy.rs) and keeps that normalized copy
 * in `PubSub.configs`. Both config read endpoints serve from there, so they
 * return fractions:
 *
 *   GET /api                    -> StreamerState.config.config   fractions
 *   GET /api/config/presets     -> every value                   fractions
 *
 * Writes go to `PubSub.config`, which holds the raw file values, so every
 * POST/PUT body must be 0..100.
 *
 * Exactly six fields are scaled:
 *   default.min_percentage, default.max_percentage, default.points.percent
 *   detailed[].threshold, detailed[].attempt_rate, detailed[].points.percent
 *
 * `points.max_value` is a point count and is never scaled. `DelayPercentage` is
 * also never scaled - the backend divides it at use time - so filters pass
 * through untouched.
 */

import type { Points, StreamerConfig } from "./types";

/** 0.9 * 100 is 90.00000000000001, so round away the float noise. */
function toDisplayPercent(fraction: number): number {
  return Math.round(fraction * 1e6) / 1e4;
}

function pointsFromWire(points: Points): Points {
  return { max_value: points.max_value, percent: toDisplayPercent(points.percent) };
}

/**
 * Convert a config as served by the API into the 0..100 form the UI edits.
 * Apply this at exactly two places: `AppState.streamers[*].config.config`, and
 * each value of `GET /api/config/presets`.
 */
export function configFromWire(config: StreamerConfig): StreamerConfig {
  const { detailed } = config.prediction.strategy;

  return {
    ...config,
    prediction: {
      ...config.prediction,
      strategy: {
        detailed: {
          default: {
            min_percentage: toDisplayPercent(detailed.default.min_percentage),
            max_percentage: toDisplayPercent(detailed.default.max_percentage),
            points: pointsFromWire(detailed.default.points),
          },
          detailed:
            detailed.detailed?.map((odds) => ({
              _type: odds._type,
              threshold: toDisplayPercent(odds.threshold),
              attempt_rate: toDisplayPercent(odds.attempt_rate),
              points: pointsFromWire(odds.points),
            })) ?? null,
        },
      },
    },
  };
}
