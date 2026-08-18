import { useMemo, useState } from "react";
import {
  Badge,
  Card,
  Group,
  SegmentedControl,
  SimpleGrid,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { IconPick } from "@tabler/icons-react";

import { EmptyState, ErrorState, LoadingState } from "../../components/States";
import { formatDelta, formatPoints } from "../../lib/format";
import { StreamerTodayCard } from "./StreamerTodayCard";
import { TodaySparkline } from "./TodaySparkline";
import { useDashboardData, type ChannelSummary } from "./useDashboardData";

type SortKey = "today" | "total" | "name";

function sortChannels(channels: ChannelSummary[], key: SortKey): ChannelSummary[] {
  switch (key) {
    case "today":
      return channels.toSorted((a, b) => b.gainedToday - a.gainedToday);
    case "total":
      return channels.toSorted((a, b) => b.points - a.points);
    case "name":
      return channels.toSorted((a, b) => a.name.localeCompare(b.name));
  }
}

export function DashboardPage() {
  const { data, isLoading, error } = useDashboardData();
  const [sort, setSort] = useState<SortKey>("today");
  const isSmallUp = useMediaQuery("(min-width: 48em)") ?? false;

  const channels = useMemo(() => sortChannels(data.channels, sort), [data.channels, sort]);

  if (error) return <ErrorState error={error} title="Could not load the dashboard" />;
  if (isLoading) return <LoadingState height={320} />;

  return (
    <Stack gap="md">
      <Card>
        {/* The figure needs nowhere near the full width, so the status pills sit
            beside it rather than stacked underneath. */}
        <Group justify="space-between" align="flex-start" gap="sm">
          <Stack gap={4}>
            <Text size="sm" c="dimmed">
              Points today
            </Text>
            <Text
              className="tpm-figure"
              fz={44}
              fw={700}
              lh={1.1}
              c={data.totalToday > 0 ? "teal" : undefined}
            >
              {formatDelta(data.totalToday)}
            </Text>
          </Stack>
          <Group gap="xs">
            <Badge color="red">{data.liveCount} live</Badge>
            <Badge color="violet">{data.miningNames.length} mining</Badge>
          </Group>
        </Group>

        <div style={{ height: isSmallUp ? 200 : 140, marginTop: 12 }}>
          <TodaySparkline data={data.sparkline} />
        </div>
      </Card>

      <Card>
        <Group gap={8} mb="xs">
          {/* Brass only while something is actually being watched, so the icon
              never implies activity that is not happening. */}
          <IconPick
            size={18}
            color={data.miningNames.length ? "var(--mantine-primary-color-filled)" : undefined}
          />
          <Title order={5}>Mining now</Title>
        </Group>
        {data.miningNames.length === 0 ? (
          <Text size="sm" c="dimmed">
            Nothing is being watched right now.
          </Text>
        ) : (
          <Stack gap={6}>
            {data.miningNames.map((name) => {
              const channel = data.channels.find((c) => c.name === name);
              return (
                <Group key={name} justify="space-between" wrap="nowrap">
                  <Text size="sm" fw={500} truncate>
                    {name}
                  </Text>
                  {channel && (
                    <Text className="tpm-num" size="sm" c="dimmed">
                      {formatPoints(channel.points)}
                    </Text>
                  )}
                </Group>
              );
            })}
          </Stack>
        )}
      </Card>

      <Stack gap="xs">
        <Group justify="space-between" wrap="wrap" gap="xs">
          <Title order={5}>Channels</Title>
          <SegmentedControl
            size="xs"
            value={sort}
            onChange={(value) => setSort(value as SortKey)}
            data={[
              { label: "Today", value: "today" },
              { label: "Total", value: "total" },
              { label: "A-Z", value: "name" },
            ]}
          />
        </Group>

        {channels.length === 0 ? (
          <EmptyState message="No streamers configured yet. Add one from the Setup page." />
        ) : (
          <SimpleGrid cols={{ base: 1, xs: 2, md: 3, lg: 4 }} spacing="sm">
            {channels.map((channel) => (
              <StreamerTodayCard key={channel.id} channel={channel} />
            ))}
          </SimpleGrid>
        )}
      </Stack>
    </Stack>
  );
}
