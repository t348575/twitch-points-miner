import { Card, Group, Stack, Text } from "@mantine/core";
import { IconCoins } from "@tabler/icons-react";
import { useNavigate } from "react-router-dom";

import { LiveDot, MiningIcon, PointsDelta } from "../../components/StatusIndicators";
import { formatPoints } from "../../lib/format";
import type { ChannelSummary } from "./useDashboardData";

export function StreamerTodayCard({ channel }: { channel: ChannelSummary }) {
  const navigate = useNavigate();

  return (
    <Card
      padding="sm"
      className="tpm-card-link"
      role="link"
      tabIndex={0}
      style={{ cursor: "pointer" }}
      onClick={() => navigate(`/history?channels=${channel.id}`)}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          navigate(`/history?channels=${channel.id}`);
        }
      }}
    >
      <Stack gap={6}>
        <Group justify="space-between" wrap="nowrap" gap="xs">
          <Text fw={600} size="sm" truncate>
            {channel.name}
          </Text>
          <Group gap={6} wrap="nowrap">
            {channel.mining && <MiningIcon />}
            {channel.live && <LiveDot />}
          </Group>
        </Group>

        <Group justify="space-between" wrap="nowrap" gap="xs">
          <Group gap={4} wrap="nowrap">
            <IconCoins size={14} opacity={0.6} />
            <Text className="tpm-num" size="sm">
              {formatPoints(channel.points)}
            </Text>
          </Group>
          <PointsDelta value={channel.gainedToday} />
        </Group>

        {channel.game && (
          <Text size="xs" c="dimmed" truncate>
            {channel.game}
          </Text>
        )}
      </Stack>
    </Card>
  );
}
