import { useMemo, useState } from "react";
import {
  Card,
  Group,
  MultiSelect,
  SegmentedControl,
  SimpleGrid,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { DatePickerInput } from "@mantine/dates";
import { useMediaQuery } from "@mantine/hooks";
import { useSearchParams } from "react-router-dom";

import { useAppState, useTimeline } from "../../api/queries";
import { daysAgo, hoursAgo, parseNaiveLocal, startOfYear, toRfc3339 } from "../../api/time";
import { EmptyState, ErrorState, LoadingState } from "../../components/States";
import { PointsDelta } from "../../components/StatusIndicators";
import { channelColor } from "../../lib/colors";
import { HistoryChart, type HistoryMode } from "./HistoryChart";

type RangePreset = "24h" | "7d" | "30d" | "90d" | "180d" | "ytd" | "custom";

/**
 * The timeline query must start before the visible range: `difference` is a LAG
 * computed inside the requested window, so the first visible row only has a
 * delta if its predecessor was fetched too.
 */
const CONTEXT_MS = 24 * 60 * 60 * 1000;

/** Parse a YYYY-MM-DD picker value as local midnight. */
function startOfDay(value: string): Date {
  const [year, month, day] = value.split("-").map(Number);
  return new Date(year, month - 1, day);
}

function endOfDay(value: string): Date {
  const date = startOfDay(value);
  date.setHours(23, 59, 59, 999);
  return date;
}

function presetStart(preset: Exclude<RangePreset, "custom">): Date {
  switch (preset) {
    case "24h":
      return hoursAgo(24);
    case "7d":
      return daysAgo(7);
    case "30d":
      return daysAgo(30);
    case "90d":
      return daysAgo(90);
    case "180d":
      return daysAgo(180);
    case "ytd":
      return startOfYear();
  }
}

export function HistoryPage() {
  const appState = useAppState();
  const [searchParams, setSearchParams] = useSearchParams();
  const isSmallUp = useMediaQuery("(min-width: 48em)") ?? false;

  const [preset, setPreset] = useState<RangePreset>("7d");
  // Mantine emits YYYY-MM-DD strings, not Date objects.
  const [customRange, setCustomRange] = useState<[string | null, string | null]>([null, null]);
  const [mode, setMode] = useState<HistoryMode>("perChannel");

  const allChannels = useMemo(() => {
    const streamers = appState.data?.streamers ?? {};
    return Object.entries(streamers)
      .map(([id, s]) => ({ id: Number(id), name: s.info.channelName, points: s.points }))
      .toSorted((a, b) => b.points - a.points);
  }, [appState.data]);

  const names = useMemo(() => new Map(allChannels.map((c) => [c.id, c.name])), [allChannels]);

  // Channel selection lives in the URL so dashboard cards can deep-link here.
  const selected = useMemo(() => {
    const param = searchParams.get("channels");
    if (param) {
      const ids = param
        .split(",")
        .map(Number)
        .filter((id) => names.has(id));
      if (ids.length) return ids;
    }
    return allChannels.slice(0, 5).map((c) => c.id);
  }, [searchParams, allChannels, names]);

  const setSelected = (ids: number[]) => {
    const next = new URLSearchParams(searchParams);
    if (ids.length) next.set("channels", ids.join(","));
    else next.delete("channels");
    setSearchParams(next, { replace: true });
  };

  const { since, request } = useMemo(() => {
    const custom = preset === "custom";
    const start =
      custom && customRange[0] ? startOfDay(customRange[0]) : presetStart(custom ? "7d" : preset);
    // The picker returns a calendar day; include all of it.
    const end = custom && customRange[1] ? endOfDay(customRange[1]) : new Date();

    return {
      since: start,
      request: {
        from: toRfc3339(new Date(start.getTime() - CONTEXT_MS)),
        to: toRfc3339(end),
        channels: selected,
      },
    };
  }, [preset, customRange, selected]);

  const timeline = useTimeline(request);

  const totals = useMemo(() => {
    const rows = timeline.data ?? [];
    const map = new Map<number, number>();
    for (const row of rows) {
      if (row.difference == null) continue;
      if (parseNaiveLocal(row.point.created_at) < since) continue;
      map.set(row.point.channel_id, (map.get(row.point.channel_id) ?? 0) + row.difference);
    }
    return [...map.entries()]
      .map(([id, gain]) => ({ id, name: names.get(id) ?? String(id), gain }))
      .toSorted((a, b) => b.gain - a.gain);
  }, [timeline.data, since, names]);

  if (appState.error) return <ErrorState error={appState.error} title="Could not load channels" />;

  return (
    <Stack gap="md">
      <Title order={4}>History</Title>

      <Card>
        <Stack gap="sm">
          <Group gap="sm" wrap="wrap">
            <SegmentedControl
              size="xs"
              value={preset}
              onChange={(value) => setPreset(value as RangePreset)}
              data={[
                { label: "24h", value: "24h" },
                { label: "7d", value: "7d" },
                { label: "30d", value: "30d" },
                { label: "90d", value: "90d" },
                { label: "180d", value: "180d" },
                { label: "YTD", value: "ytd" },
                { label: "Custom", value: "custom" },
              ]}
            />
            <SegmentedControl
              size="xs"
              value={mode}
              onChange={(value) => setMode(value as HistoryMode)}
              data={[
                { label: "Per channel", value: "perChannel" },
                { label: "Total", value: "total" },
              ]}
            />
          </Group>

          {preset === "custom" && (
            <DatePickerInput
              type="range"
              size="sm"
              label="Date range"
              placeholder="Pick a start and end date"
              value={customRange}
              onChange={setCustomRange}
              maxDate={new Date()}
              clearable
            />
          )}

          <MultiSelect
            size="sm"
            label="Channels"
            placeholder={selected.length ? undefined : "Pick at least one channel"}
            searchable
            clearable
            data={allChannels.map((c) => ({ value: String(c.id), label: c.name }))}
            value={selected.map(String)}
            onChange={(values) => setSelected(values.map(Number))}
          />
        </Stack>
      </Card>

      <Card>
        {timeline.error ? (
          <ErrorState error={timeline.error} title="Could not load history" />
        ) : selected.length === 0 ? (
          <EmptyState message="Pick at least one channel to see its history." />
        ) : timeline.isLoading ? (
          <LoadingState height={280} />
        ) : (
          <HistoryChart
            rows={timeline.data ?? []}
            mode={mode}
            names={names}
            since={since}
            height={isSmallUp ? 420 : 280}
            showSlider={isSmallUp}
          />
        )}
      </Card>

      {totals.length > 0 && (
        <Stack gap="xs">
          <Title order={5}>Gained in this range</Title>
          <SimpleGrid cols={{ base: 1, xs: 2, md: 3 }} spacing="sm">
            {totals.map((entry) => (
              <Card key={entry.id} padding="sm">
                <Group justify="space-between" wrap="nowrap">
                  <Group gap={8} wrap="nowrap">
                    <span
                      style={{
                        width: 8,
                        height: 8,
                        borderRadius: "50%",
                        background: channelColor(entry.id),
                        flexShrink: 0,
                      }}
                    />
                    <Text size="sm" truncate>
                      {entry.name}
                    </Text>
                  </Group>
                  <PointsDelta value={entry.gain} />
                </Group>
              </Card>
            ))}
          </SimpleGrid>
        </Stack>
      )}
    </Stack>
  );
}
