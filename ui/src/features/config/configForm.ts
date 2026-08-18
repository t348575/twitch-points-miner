/**
 * Form model for a streamer/preset config.
 *
 * The form always holds display values (0..100), which is also the wire format
 * for writes. `configFromWire` handles the read direction once, in the API layer.
 *
 * Filters are flattened into `{kind, value}` rows because the API shape
 * (`{TotalUsers: 300}`) is awkward to bind inputs to.
 */

import type { ConfigType, DetailedOdds, Filter, FilterKind, StreamerConfig } from "../../api/types";

export interface FilterRow {
  /** Stable React key. Stripped before the config is sent. */
  id: string;
  kind: FilterKind;
  value: number;
}

/** A detailed odds rule plus a stable React key. */
export type OddsRow = DetailedOdds & { id: string };

let rowCounter = 0;
export function rowId(): string {
  rowCounter += 1;
  return `row-${rowCounter}`;
}

export interface ConfigFormValues {
  mode: "preset" | "specific";
  presetName: string;
  follow_raid: boolean;
  min_percentage: number;
  max_percentage: number;
  default_max_value: number;
  default_percent: number;
  detailed: OddsRow[];
  filters: FilterRow[];
}

export function filterToRow(filter: Filter): FilterRow {
  if ("TotalUsers" in filter) return { id: rowId(), kind: "TotalUsers", value: filter.TotalUsers };
  if ("DelaySeconds" in filter)
    return { id: rowId(), kind: "DelaySeconds", value: filter.DelaySeconds };
  return { id: rowId(), kind: "DelayPercentage", value: filter.DelayPercentage };
}

export function rowToFilter(row: FilterRow): Filter {
  switch (row.kind) {
    case "TotalUsers":
      return { TotalUsers: row.value };
    case "DelaySeconds":
      return { DelaySeconds: row.value };
    case "DelayPercentage":
      return { DelayPercentage: row.value };
  }
}

/** A blank rule, with a fresh key each time. */
export function emptyOddsRow(): OddsRow {
  return {
    id: rowId(),
    _type: "Ge",
    threshold: 80,
    attempt_rate: 100,
    points: { max_value: 1000, percent: 1 },
  };
}

/** A blank filter, with a fresh key each time. */
export function emptyFilterRow(): FilterRow {
  return { id: rowId(), kind: "TotalUsers", value: 100 };
}

export function emptyConfig(): StreamerConfig {
  return {
    follow_raid: true,
    prediction: {
      strategy: {
        detailed: {
          detailed: null,
          default: { min_percentage: 40, max_percentage: 60, points: { max_value: 0, percent: 0 } },
        },
      },
      filters: [],
    },
  };
}

/** Build form values from a config already converted to display scale. */
export function toFormValues(
  config: StreamerConfig,
  mode: "preset" | "specific",
  presetName: string,
): ConfigFormValues {
  const { detailed } = config.prediction.strategy;
  return {
    mode,
    presetName,
    follow_raid: config.follow_raid,
    min_percentage: detailed.default.min_percentage,
    max_percentage: detailed.default.max_percentage,
    default_max_value: detailed.default.points.max_value,
    default_percent: detailed.default.points.percent,
    detailed:
      detailed.detailed?.map((odds) => ({
        id: rowId(),
        _type: odds._type,
        threshold: odds.threshold,
        attempt_rate: odds.attempt_rate,
        points: { max_value: odds.points.max_value, percent: odds.points.percent },
      })) ?? [],
    filters: config.prediction.filters.map(filterToRow),
  };
}

/** Build the request body. Values are already 0..100, which is what writes expect. */
export function toStreamerConfig(values: ConfigFormValues): StreamerConfig {
  return {
    follow_raid: values.follow_raid,
    prediction: {
      strategy: {
        detailed: {
          // The backend takes Option<Vec<_>>; send null rather than an empty list
          // to match how the config file is normally written. The `id` is a
          // local React key and must not reach the API.
          detailed: values.detailed.length
            ? values.detailed.map(({ id: _id, ...odds }) => odds)
            : null,
          default: {
            min_percentage: values.min_percentage,
            max_percentage: values.max_percentage,
            points: { max_value: values.default_max_value, percent: values.default_percent },
          },
        },
      },
      filters: values.filters.map(rowToFilter),
    },
  };
}

export function toConfigType(values: ConfigFormValues): ConfigType {
  if (values.mode === "preset") return { Preset: values.presetName };
  return { Specific: toStreamerConfig(values) };
}

/** Mirrors the `#[validate(range(min = 0.0, max = 100.0))]` attributes in Rust. */
export function validateConfigForm(values: ConfigFormValues) {
  const errors: Record<string, string> = {};

  if (values.mode === "preset") {
    if (!values.presetName) errors.presetName = "Pick a preset";
    return errors;
  }

  const pct = (v: number, field: string) => {
    if (v < 0 || v > 100 || Number.isNaN(v)) errors[field] = "Must be between 0 and 100";
  };

  pct(values.min_percentage, "min_percentage");
  pct(values.max_percentage, "max_percentage");
  pct(values.default_percent, "default_percent");

  if (values.min_percentage > values.max_percentage) {
    // The backend does not check this, but it can never match anything.
    errors.min_percentage = "Minimum cannot be above the maximum";
  }
  if (values.default_max_value < 0) errors.default_max_value = "Cannot be negative";

  values.detailed.forEach((odds, index) => {
    pct(odds.threshold, `detailed.${index}.threshold`);
    pct(odds.attempt_rate, `detailed.${index}.attempt_rate`);
    pct(odds.points.percent, `detailed.${index}.points.percent`);
    if (odds.points.max_value < 0)
      errors[`detailed.${index}.points.max_value`] = "Cannot be negative";
  });

  values.filters.forEach((filter, index) => {
    if (filter.value < 0) errors[`filters.${index}.value`] = "Cannot be negative";
    if (filter.kind === "DelayPercentage" && filter.value > 100) {
      errors[`filters.${index}.value`] = "Must be between 0 and 100";
    }
  });

  return errors;
}
