/**
 * TanStack Query keys, hooks and invalidation rules.
 *
 * There is no websocket or SSE on the backend, so freshness comes from polling.
 * Intervals only run while the tab is visible (TanStack's default), which keeps
 * phones from burning battery in the background.
 */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { QueryClient } from "@tanstack/react-query";

import * as api from "./endpoints";
import type { ConfigType, MakePrediction, StreamerConfig, TimelineRequest } from "./types";

export const keys = {
  appState: ["appState"] as const,
  live: ["live"] as const,
  presets: ["presets"] as const,
  watchPriority: ["watchPriority"] as const,
  maxWatching: ["maxWatching"] as const,
  watchStreak: ["watchStreak"] as const,
  timeline: (req: TimelineRequest) =>
    ["timeline", req.from, req.to, req.channels.join(",")] as const,
  logs: (page: number, perPage: number) => ["logs", page, perPage] as const,
};

/* ---------- reads ---------- */

export function useAppState() {
  return useQuery({
    queryKey: keys.appState,
    queryFn: ({ signal }) => api.getAppState(signal),
    refetchInterval: 15_000,
  });
}

export function useLiveStreamers(enabled = true) {
  return useQuery({
    queryKey: keys.live,
    queryFn: ({ signal }) => api.getLiveStreamers(signal),
    refetchInterval: 10_000,
    enabled,
  });
}

export function useTimeline(req: TimelineRequest, options?: { refetchInterval?: number | false }) {
  return useQuery({
    queryKey: keys.timeline(req),
    queryFn: ({ signal }) => api.postTimeline(req, signal),
    refetchInterval: options?.refetchInterval ?? false,
    enabled: req.channels.length > 0,
    // Keep the previous chart on screen while a new range loads.
    placeholderData: (prev) => prev,
  });
}

export function usePresets() {
  return useQuery({ queryKey: keys.presets, queryFn: ({ signal }) => api.getPresets(signal) });
}

export function useWatchPriority() {
  return useQuery({
    queryKey: keys.watchPriority,
    queryFn: ({ signal }) => api.getWatchPriority(signal),
  });
}

export function useMaxWatching() {
  return useQuery({
    queryKey: keys.maxWatching,
    queryFn: ({ signal }) => api.getMaxWatching(signal),
  });
}

export function useWatchStreak() {
  return useQuery({
    queryKey: keys.watchStreak,
    queryFn: ({ signal }) => api.getWatchStreak(signal),
  });
}

export function useLogs(page: number, perPage: number, autoRefresh: boolean) {
  return useQuery({
    queryKey: keys.logs(page, perPage),
    queryFn: ({ signal }) => api.getLogs(page, perPage, signal),
    refetchInterval: autoRefresh ? 10_000 : false,
  });
}

/* ---------- writes ---------- */

function invalidate(client: QueryClient, ...queryKeys: readonly (readonly unknown[])[]) {
  return Promise.all(queryKeys.map((queryKey) => client.invalidateQueries({ queryKey })));
}

export function useMineStreamer() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ name, config }: { name: string; config: ConfigType }) =>
      api.mineStreamer(name, config),
    onSuccess: () => invalidate(client, keys.appState, keys.live),
  });
}

export function useRemoveStreamer() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => api.removeStreamer(name),
    // Removing a streamer can also drop it from the watch priority list.
    onSuccess: () => invalidate(client, keys.appState, keys.live, keys.watchPriority),
  });
}

export function useSaveStreamerConfig() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ name, config }: { name: string; config: ConfigType }) =>
      api.saveStreamerConfig(name, config),
    onSuccess: () => invalidate(client, keys.appState),
  });
}

export function useSavePreset() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ name, config }: { name: string; config: StreamerConfig }) =>
      api.savePreset(name, config),
    // A preset edit mutates the shared config of every streamer using it.
    onSuccess: () => invalidate(client, keys.presets, keys.appState),
  });
}

export function useDeletePreset() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => api.deletePreset(name),
    onSuccess: () => invalidate(client, keys.presets, keys.appState),
  });
}

export function useSetWatchPriority() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (priority: string[]) => api.setWatchPriority(priority),
    onSuccess: () => invalidate(client, keys.watchPriority, keys.appState),
  });
}

export function useSetMaxWatching() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (value: number) => api.setMaxWatching(value),
    onSuccess: () => invalidate(client, keys.maxWatching, keys.appState),
  });
}

export function useSetWatchStreak() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (value: boolean) => api.setWatchStreak(value),
    onSuccess: () => invalidate(client, keys.watchStreak),
  });
}

export function usePlaceBet() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ streamer, body }: { streamer: string; body: MakePrediction }) =>
      api.placeBet(streamer, body),
    onSuccess: () => invalidate(client, keys.live, keys.appState),
  });
}
