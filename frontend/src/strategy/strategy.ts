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

export const DETAILED_STRATEGY_ODDS_COMPARISON_TYPES = [
  { value: "Le", label: "<= LE" },
  { value: "Ge", label: ">= GE" },
];
