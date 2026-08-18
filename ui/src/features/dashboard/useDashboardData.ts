import { useMemo } from "react";

import { useAppState, useMaxWatching, useTimeline } from "../../api/queries";
import { daysAgo, parseNaiveLocal, startOfToday, toRfc3339 } from "../../api/time";
import type { TimelineResult } from "../../api/types";

export interface ChannelSummary {
  id: number;
  name: string;
  points: number;
  gainedToday: number;
  live: boolean;
  mining: boolean;
  game: string | null;
}

export interface DashboardData {
  channels: ChannelSummary[];
  totalToday: number;
  liveCount: number;
  miningNames: string[];
  /** Cumulative points gained today, as [timestamp, value] pairs. */
  sparkline: [number, number][];
}

/**
 * How many days of history to request. We only display today, but the query has
 * to start before midnight: `difference` is a SQL LAG computed *within the
 * requested window*, so today's first row only gets a non-null delta if its
 * predecessor is included. Seven days covers channels that were idle for a while.
 */
const LOOKBACK_DAYS = 7;

/**
 * Sum of the day's deltas for one channel.
 *
 * Using the backend's `difference` avoids the bug in the old UI, which did
 * `last - first` of today's rows: that dropped the overnight gain and reported
 * zero for any channel with a single row today. Rows with a null difference are
 * skipped - only the very first row of the whole window has one, and it is a
 * baseline rather than a gain.
 */
function gainSince(rows: TimelineResult[], since: Date): number {
  let total = 0;
  for (const row of rows) {
    if (row.difference == null) continue;
    if (parseNaiveLocal(row.point.created_at) < since) continue;
    total += row.difference;
  }
  return total;
}

export function useDashboardData() {
  const appState = useAppState();
  const maxWatching = useMaxWatching();

  const channelIds = useMemo(
    () => Object.keys(appState.data?.streamers ?? {}).map(Number),
    [appState.data],
  );

  const range = useMemo(() => {
    const today = startOfToday();
    return {
      from: toRfc3339(daysAgo(LOOKBACK_DAYS, today)),
      to: toRfc3339(new Date()),
      channels: channelIds,
    };
    // `to` is "now"; recompute only when the channel set changes so the query
    // key stays stable and polling drives the refresh instead.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [channelIds.join(",")]);

  const timeline = useTimeline(range, { refetchInterval: 60_000 });

  const data = useMemo<DashboardData>(() => {
    const state = appState.data;
    const rows = timeline.data ?? [];
    const today = startOfToday();

    if (!state) {
      return { channels: [], totalToday: 0, liveCount: 0, miningNames: [], sparkline: [] };
    }

    // The backend only actually mines the first `max_watching` entries.
    const limit = maxWatching.data ?? 2;
    const miningNames = state.watching.slice(0, limit).map((s) => s.info.channelName);
    const miningSet = new Set(miningNames);

    const rowsByChannel = new Map<number, TimelineResult[]>();
    for (const row of rows) {
      const list = rowsByChannel.get(row.point.channel_id);
      if (list) list.push(row);
      else rowsByChannel.set(row.point.channel_id, [row]);
    }

    const channels: ChannelSummary[] = Object.entries(state.streamers).map(([id, streamer]) => {
      const channelId = Number(id);
      return {
        id: channelId,
        name: streamer.info.channelName,
        points: streamer.points,
        gainedToday: gainSince(rowsByChannel.get(channelId) ?? [], today),
        live: streamer.info.live,
        mining: miningSet.has(streamer.info.channelName),
        game: streamer.info.game?.name ?? null,
      };
    });

    const totalToday = channels.reduce((sum, c) => sum + c.gainedToday, 0);

    // Cumulative gain across all channels, seeded at zero at midnight.
    const todayRows = rows
      .filter((r) => r.difference != null && parseNaiveLocal(r.point.created_at) >= today)
      .toSorted(
        (a, b) =>
          parseNaiveLocal(a.point.created_at).getTime() -
          parseNaiveLocal(b.point.created_at).getTime(),
      );

    const sparkline: [number, number][] = [[today.getTime(), 0]];
    let running = 0;
    for (const row of todayRows) {
      running += row.difference ?? 0;
      sparkline.push([parseNaiveLocal(row.point.created_at).getTime(), running]);
    }

    return {
      channels,
      totalToday,
      liveCount: channels.filter((c) => c.live).length,
      miningNames,
      sparkline,
    };
  }, [appState.data, timeline.data, maxWatching.data]);

  return {
    data,
    isLoading: appState.isLoading || timeline.isLoading,
    error: appState.error ?? timeline.error,
  };
}
