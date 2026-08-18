/**
 * One function per backend route.
 *
 * Several routes need a trailing slash and several take a bare JSON scalar as
 * the body. Both quirks are encoded here so no call site has to remember them.
 */

import { requestEmpty, requestJson, requestStatus, requestText } from "./http";
import { configFromWire } from "./normalize";
import type {
  AppState,
  BetOutcome,
  ConfigType,
  LiveStreamer,
  MakePrediction,
  AnalyticsPrediction,
  Presets,
  StreamerConfig,
  StreamerState,
  TimelineRequest,
  TimelineResult,
} from "./types";

/* ---------- app state ---------- */

export async function getAppState(signal?: AbortSignal): Promise<AppState> {
  const state = await requestJson<AppState>("/api", { signal });

  // Configs arrive with percentages already divided by 100.
  for (const streamer of Object.values(state.streamers)) {
    streamer.config.config = configFromWire(streamer.config.config);
  }
  for (const wrapper of Object.values(state.configs)) {
    wrapper.config = configFromWire(wrapper.config);
  }
  for (const watched of state.watching) {
    watched.config.config = configFromWire(watched.config.config);
  }

  return state;
}

/* ---------- streamers ---------- */

export function getLiveStreamers(signal?: AbortSignal): Promise<LiveStreamer[]> {
  return requestJson<LiveStreamer[]>("/api/streamers/live", { signal });
}

export function getStreamer(name: string, signal?: AbortSignal): Promise<StreamerState> {
  return requestJson<StreamerState>(`/api/streamers/${encodeURIComponent(name)}`, { signal });
}

export function mineStreamer(channelName: string, config: ConfigType): Promise<void> {
  return requestEmpty(`/api/streamers/mine/${encodeURIComponent(channelName)}`, {
    method: "PUT",
    body: { config },
  });
}

export function removeStreamer(channelName: string): Promise<void> {
  // Trailing slash is required by the router.
  return requestEmpty(`/api/streamers/mine/${encodeURIComponent(channelName)}/`, {
    method: "DELETE",
  });
}

/* ---------- predictions ---------- */

export function getLivePrediction(
  channelId: number,
  predictionId: string,
  signal?: AbortSignal,
): Promise<AnalyticsPrediction | null> {
  const query = new URLSearchParams({
    prediction_id: predictionId,
    channel_id: String(channelId),
  });
  return requestJson<AnalyticsPrediction | null>(`/api/predictions/live?${query}`, { signal });
}

/**
 * Place a bet. The backend answers 201 when it placed one and 202 when the
 * configured strategy declined, which is a normal outcome rather than an error.
 */
export async function placeBet(streamer: string, body: MakePrediction): Promise<BetOutcome> {
  const status = await requestStatus(`/api/predictions/bet/${encodeURIComponent(streamer)}`, {
    method: "POST",
    body,
  });
  return status === 201 ? "placed" : "declined";
}

/* ---------- config ---------- */

export async function getPresets(signal?: AbortSignal): Promise<Presets> {
  const presets = await requestJson<Presets>("/api/config/presets", { signal });
  return Object.fromEntries(
    Object.entries(presets).map(([name, config]) => [name, configFromWire(config)]),
  );
}

export function savePreset(name: string, config: StreamerConfig): Promise<void> {
  // Trailing slash is required by the router.
  return requestEmpty("/api/config/presets/", { method: "POST", body: { name, config } });
}

export function deletePreset(name: string): Promise<void> {
  return requestEmpty(`/api/config/presets/${encodeURIComponent(name)}`, { method: "DELETE" });
}

export function saveStreamerConfig(channelName: string, config: ConfigType): Promise<void> {
  return requestEmpty(`/api/config/streamer/${encodeURIComponent(channelName)}`, {
    method: "POST",
    body: config,
  });
}

export function getWatchPriority(signal?: AbortSignal): Promise<string[]> {
  return requestJson<string[]>("/api/config/watch_priority", { signal });
}

export function setWatchPriority(priority: string[]): Promise<void> {
  // Trailing slash is required by the router.
  return requestEmpty("/api/config/watch_priority/", { method: "POST", body: priority });
}

export function getMaxWatching(signal?: AbortSignal): Promise<number> {
  return requestJson<number>("/api/config/max_watching", { signal });
}

export function setMaxWatching(maxWatching: number): Promise<void> {
  // Trailing slash, and the body is a bare JSON number.
  return requestEmpty("/api/config/max_watching/", { method: "POST", body: maxWatching });
}

export function getWatchStreak(signal?: AbortSignal): Promise<boolean> {
  return requestJson<boolean>("/api/config/watch_streak", { signal });
}

export function setWatchStreak(enabled: boolean): Promise<void> {
  // Trailing slash, and the body is a bare JSON boolean.
  return requestEmpty("/api/config/watch_streak/", { method: "POST", body: enabled });
}

/* ---------- analytics ---------- */

export function postTimeline(
  body: TimelineRequest,
  signal?: AbortSignal,
): Promise<TimelineResult[]> {
  return requestJson<TimelineResult[]>("/api/analytics/timeline", {
    method: "POST",
    body,
    signal,
  });
}

/* ---------- logs ---------- */

/** Returns server-rendered HTML (ANSI colours already converted). */
export function getLogs(page: number, perPage: number, signal?: AbortSignal): Promise<string> {
  const query = new URLSearchParams({ page: String(page), per_page: String(perPage) });
  return requestText(`/api/logs?${query}`, { signal });
}
