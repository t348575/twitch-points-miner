import { useMemo, useState } from "react";
import { ActionIcon, Badge, Button, Card, Group, Stack, Text } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { modals } from "@mantine/modals";
import { notifications } from "@mantine/notifications";
import { IconPlus, IconSettings, IconTrash } from "@tabler/icons-react";

import { ApiError } from "../../api/http";
import { useAppState, usePresets, useRemoveStreamer } from "../../api/queries";
import { EmptyState, ErrorState, LoadingState } from "../../components/States";
import { LiveDot } from "../../components/StatusIndicators";
import { formatPoints } from "../../lib/format";
import type { StreamerConfigRefWrapper } from "../../api/types";
import { StreamerConfigModal } from "./StreamerConfigModal";

interface Row {
  name: string;
  points: number;
  live: boolean;
  config: StreamerConfigRefWrapper;
}

export function StreamersTab() {
  const appState = useAppState();
  const presets = usePresets();
  const removeStreamer = useRemoveStreamer();
  const [opened, { open, close }] = useDisclosure(false);
  const [editing, setEditing] = useState<Row | null>(null);

  const rows = useMemo<Row[]>(() => {
    const streamers = appState.data?.streamers ?? {};
    return Object.values(streamers)
      .map((s) => ({
        name: s.info.channelName,
        points: s.points,
        live: s.info.live,
        config: s.config,
      }))
      .toSorted((a, b) => a.name.localeCompare(b.name));
  }, [appState.data]);

  const presetNames = useMemo(() => Object.keys(presets.data ?? {}), [presets.data]);

  const confirmRemove = (row: Row) =>
    modals.openConfirmModal({
      title: `Stop mining ${row.name}?`,
      children: <Text size="sm">This removes the streamer from your config file.</Text>,
      labels: { confirm: "Remove", cancel: "Cancel" },
      confirmProps: { color: "red" },
      onConfirm: async () => {
        try {
          await removeStreamer.mutateAsync(row.name);
          notifications.show({ color: "teal", message: `Removed ${row.name}` });
        } catch (error) {
          notifications.show({
            color: "red",
            title: "Could not remove streamer",
            message: error instanceof ApiError ? error.message : String(error),
          });
        }
      },
    });

  if (appState.error) return <ErrorState error={appState.error} />;
  if (appState.isLoading) return <LoadingState />;

  return (
    <Stack gap="sm">
      <Group justify="space-between">
        <Text size="sm" c="dimmed">
          {rows.length} streamer{rows.length === 1 ? "" : "s"}
        </Text>
        <Button
          size="xs"
          leftSection={<IconPlus size={14} />}
          onClick={() => {
            setEditing(null);
            open();
          }}
        >
          Add streamer
        </Button>
      </Group>

      {rows.length === 0 ? (
        <EmptyState message="No streamers configured yet." />
      ) : (
        <Stack gap="xs">
          {rows.map((row) => {
            const ref = row.config._type;
            const label = typeof ref === "string" ? "Custom" : `Preset: ${ref.Preset}`;

            return (
              <Card key={row.name} padding="sm">
                <Group justify="space-between" wrap="nowrap" gap="xs">
                  <Stack gap={2} style={{ minWidth: 0 }}>
                    <Group gap={6} wrap="nowrap">
                      <Text fw={600} size="sm" truncate>
                        {row.name}
                      </Text>
                      {row.live && <LiveDot />}
                    </Group>
                    <Group gap={6}>
                      <Text className="tpm-num" size="xs" c="dimmed">
                        {formatPoints(row.points)} points
                      </Text>
                      <Badge size="xs" variant="light">
                        {label}
                      </Badge>
                    </Group>
                  </Stack>

                  <Group gap={4} wrap="nowrap">
                    <ActionIcon
                      variant="subtle"
                      aria-label={`Configure ${row.name}`}
                      onClick={() => {
                        setEditing(row);
                        open();
                      }}
                    >
                      <IconSettings size={16} />
                    </ActionIcon>
                    <ActionIcon
                      variant="subtle"
                      color="red"
                      aria-label={`Remove ${row.name}`}
                      onClick={() => confirmRemove(row)}
                    >
                      <IconTrash size={16} />
                    </ActionIcon>
                  </Group>
                </Group>
              </Card>
            );
          })}
        </Stack>
      )}

      <StreamerConfigModal
        opened={opened}
        onClose={close}
        presetNames={presetNames}
        streamer={editing}
      />
    </Stack>
  );
}
