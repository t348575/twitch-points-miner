import createClient from "openapi-fetch";
import type { components, paths } from "./api";

const client = createClient<paths>({
  baseUrl: import.meta.env.DEV ? "http://localhost:3000" : window.location.href,
});

export interface Streamer {
  id: number;
  data: components["schemas"]["StreamerState"];
}

export interface ValidateStrategy {
  status: boolean;
  data: components["schemas"]["Strategy"];
}

export interface FilterType {
  value: string;
  label: string;
  quantity: number;
}

export function default_validate_strategy(obj: any): string | undefined {
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
    if (obj[v] == undefined || obj[v] < 0.0 || obj[v] > 100.0) {
      return `Invalid ${v.split("_").join(" ")}`;
    }
  }
}

function default_strategy_apply_function(
  obj: any,
  func: { (item: any): any }
): any {
  let obj_copy = JSON.parse(JSON.stringify(obj));
  for (const v of Object.keys(obj)) {
    // points object
    if (typeof obj[v] == "object") {
      obj_copy[v].percent = func(obj[v].percent * 100.0);
      obj_copy[v].max_value = func(obj[v].max_value);
    } else {
      obj_copy[v] = func(obj[v] * 100.0);
    }
  }
  return obj_copy;
}

export function default_parse_strategy(obj: any): any {
  return default_strategy_apply_function(obj, parseFloat);
}

export function default_stringify_strategy(obj: any): any {
  return default_strategy_apply_function(obj, (x) => x.toString());
}

export async function get_streamers(): Promise<Streamer[]> {
  const { data, error } = await client.GET("/api");
  if (error) {
    throw error;
  }

  let items: Streamer[] = [];
  for (const v of Object.keys(data.streamers)) {
    items.push({
      id: parseInt(v, 10),
      data: data.streamers[v] as components["schemas"]["StreamerState"],
    });
  }

  return items;
}

export async function mine_streamer(
  channel_name: string,
  config: components["schemas"]["ConfigType"]
) {
  const { error } = await client.PUT("/api/streamers/mine/{channel_name}", {
    params: {
      path: {
        channel_name,
      },
    },
    body: {
      config,
    },
  });

  if (error) {
    throw error;
  }
  return;
}

export async function remove_streamer(channelName: string) {
  const { error } = await client.DELETE("/api/streamers/mine/{channel_name}/", {
    params: {
      path: {
        channel_name: channelName,
      },
    },
  });

  if (error) {
    throw error;
  }
  return;
}

export async function save_streamer_config(
  channelName: string,
  config: components["schemas"]["ConfigType"]
) {
  const { error } = await client.POST("/api/streamers/config/{channel_name}", {
    params: {
      path: {
        channel_name: channelName,
      },
    },
    body: config,
  });
}
