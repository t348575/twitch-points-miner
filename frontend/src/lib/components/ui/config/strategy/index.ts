import * as z from "zod";
import DetailedStrategy from "./DetailedStrategy.svelte";
import External from "./External.svelte";
import { typedObjectKeys } from "$common";

export function validate_detailed_strategy(obj: any): string | undefined {
  for (const v of Object.keys(obj)) {
    // points object
    if (typeof obj[v] == "object") {
      if (
        obj[v].percent == undefined ||
        obj[v].percent < 0.0 ||
        obj[v].percent > 100.0
      ) {
        return "Invalid points percentage";
      }

      if (obj[v].max_value == undefined) {
        return "Invalid max points value";
      }
    }

    if (v == "_type") {
      continue;
    }

    if (obj[v] == undefined || obj[v] < 0.0 || obj[v] > 100.0) {
      return `Invalid ${v.split("_").join(" ")}`;
    }
  }
}

function detailed_strategy_apply_function(
  obj: any,
  func: { (item: any): any },
  skip_max_value = false,
): any {
  let obj_copy = JSON.parse(JSON.stringify(obj));
  for (const v of Object.keys(obj)) {
    if (v == "_type") {
      continue;
    }

    // points object
    if (typeof obj[v] == "object") {
      obj_copy[v].percent = func(obj[v].percent);
      if (!skip_max_value) {
        obj_copy[v].max_value = func(obj[v].max_value);
      }
    } else {
      obj_copy[v] = func(obj[v]);
    }
  }
  return obj_copy;
}

export function detailed_strategy_parse(obj: any): any {
  return detailed_strategy_apply_function(obj, parseFloat);
}

export function detailed_strategy_stringify(obj: any): any {
  return detailed_strategy_apply_function(
    detailed_strategy_apply_function(obj, (x) => x * 100.0, true),
    (x) => x.toString(),
  );
}

export const DETAILED_STRATEGY_ODDS_COMPARISON_TYPES = {
  Le: "<= LE",
  Ge: ">= GE",
};

const points_schema = z.object({
  max_value: z.number().min(0),
  percent: z.number().min(0).max(100),
});

const [dsFirstType, ...dsOtherTypes] = typedObjectKeys(
  DETAILED_STRATEGY_ODDS_COMPARISON_TYPES,
);
export const dsSchema = z.object({
  default: z.object({
    max_percentage: z.number().min(0).max(100),
    min_percentage: z.number().min(0).max(100),
    points: points_schema,
  }),
  detailed: z.object({
    type: z.enum([dsFirstType!, ...dsOtherTypes]),
    attempt_rate: z.number().min(0).max(100),
    points: points_schema,
    threshold: z.number().min(0).max(100),
  }).array(),
});

export const externalSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("Inline"),
    data: z.string().min(1),
  }),
  z.object({
    type: z.literal("File"),
    data: z.string().min(1),
    file_data: z.string().optional(),
  }),
]);

export { DetailedStrategy, External };
