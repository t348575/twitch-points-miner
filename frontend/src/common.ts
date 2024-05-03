import createClient from "openapi-fetch";
import type { components, paths } from "./api";
import { writable } from "svelte/store";

export const streamers = writable<Streamer[]>([]);

const baseUrl = import.meta.env.DEV
  ? "http://localhost:3000/"
  : window.location.href;
const client = createClient<paths>({
  baseUrl,
});

export interface Streamer {
  id: number;
  data: components["schemas"]["StreamerState"];
  name: string;
  points: number;
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

export async function get_streamers(): Promise<Streamer[]> {
  const { data, error } = await client.GET("/api");
  if (error) {
    throw error;
  }

  let items: Streamer[] = [];
  for (const v in data.streamers) {
    const s = data.streamers[v] as components["schemas"]["StreamerState"];
    items.push({
      id: parseInt(v, 10),
      data: s,
      points: s.points,
      name: s.info.channelName,
    });
  }

  return items;
}

export async function mine_streamer(
  channel_name: string,
  config: components["schemas"]["ConfigType"],
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
  config: components["schemas"]["ConfigType"],
) {
  const { error } = await client.POST("/api/config/streamer/{channel_name}", {
    params: {
      path: {
        channel_name: channelName,
      },
    },
    body: config,
  });
  if (error) {
    throw error;
  }
}

export async function place_bet_streamer(
  streamer: string,
  event_id: string,
  outcome_id: string,
  points: number | null,
) {
  const { error } = await client.POST("/api/predictions/bet/{streamer}", {
    params: {
      path: {
        streamer,
      },
    },
    body: {
      event_id,
      outcome_id,
      points,
    },
  });

  if (error) {
    throw error;
  }
}

export async function get_presets(): Promise<{
  [key: string]: components["schemas"]["StreamerConfig"];
}> {
  const { data, error } = await client.GET("/api/config/presets");
  if (error) {
    throw error;
  }
  // @ts-ignore
  return data;
}

export async function add_or_update_preset(
  name: string,
  config: components["schemas"]["StreamerConfig"],
) {
  const { error } = await client.POST("/api/config/presets/", {
    body: {
      config,
      name,
    },
  });
  if (error) {
    throw error;
  }
}

export async function delete_preset(name: string) {
  const { error } = await client.DELETE("/api/config/presets/{name}", {
    params: {
      path: {
        name,
      },
    },
  });
  if (error) {
    throw error;
  }
}

export async function get_watching(): Promise<
  components["schemas"]["StreamerState"][]
> {
  const { data, error } = await client.GET("/api");
  if (error) {
    throw error;
  }
  return data.watching;
}

export async function get_timeline(
  from: string,
  to: string,
  channels: Streamer[],
): Promise<components["schemas"]["TimelineResult"][]> {
  const { data, error } = await client.POST("/api/analytics/timeline", {
    body: {
      channels: channels.map((a) => a.id),
      from,
      to,
    },
  });

  if (error) {
    throw error;
  }
  return data;
}

export async function get_live_streamers(): Promise<
  components["schemas"]["LiveStreamer"][]
> {
  const { data, error } = await client.GET("/api/streamers/live");
  if (error) {
    throw error;
  }
  return data;
}

export async function get_last_prediction(
  channel_id: number,
  prediction_id: string,
): Promise<components["schemas"]["Prediction"] | null> {
  const { data, error } = await client.GET("/api/predictions/live", {
    params: {
      query: {
        prediction_id,
        channel_id,
      },
    },
  });
  if (error) {
    throw error;
  }
  return data;
}

export async function get_watch_priority(): Promise<string[]> {
  const { data, error } = await client.GET("/api/config/watch_priority");
  if (error) {
    throw error;
  }
  return data;
}

export async function set_watch_priority(
  watch_priority: string[],
): Promise<void> {
  const { error } = await client.POST("/api/config/watch_priority/", {
    body: watch_priority,
  });
  if (error) {
    throw error;
  }
}

export async function get_logs(
  page: number,
  page_size: number,
): Promise<string> {
  const res = await fetch(
    `${baseUrl}api/logs?page=${page}&per_page=${page_size}`,
  );
  return await res.text();
}
