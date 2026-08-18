import { useMemo, useState } from "react";
import { ActionIcon, Badge, Button, Card, Group, Stack, Text } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { modals } from "@mantine/modals";
import { notifications } from "@mantine/notifications";
import { IconPencil, IconPlus, IconTrash } from "@tabler/icons-react";

import { ApiError } from "../../api/http";
import { useAppState, useDeletePreset, usePresets } from "../../api/queries";
import { EmptyState, ErrorState, LoadingState } from "../../components/States";
import type { StreamerConfig } from "../../api/types";
import { PresetModal } from "./PresetModal";

export function PresetsTab() {
  const presets = usePresets();
  const appState = useAppState();
  const deletePreset = useDeletePreset();
  const [opened, { open, close }] = useDisclosure(false);
  const [editing, setEditing] = useState<{ name: string; config: StreamerConfig } | null>(null);

  // How many streamers reference each preset, so deletion is predictable.
  const usage = useMemo(() => {
    const counts = new Map<string, number>();
    for (const streamer of Object.values(appState.data?.streamers ?? {})) {
      const ref = streamer.config._type;
      if (typeof ref !== "string") {
        counts.set(ref.Preset, (counts.get(ref.Preset) ?? 0) + 1);
      }
    }
    return counts;
  }, [appState.data]);

  const entries = useMemo(() => Object.entries(presets.data ?? {}), [presets.data]);

  const confirmDelete = (name: string) =>
    modals.openConfirmModal({
      title: `Delete preset ${name}?`,
      children: <Text size="sm">Streamers using it must be reassigned first.</Text>,
      labels: { confirm: "Delete", cancel: "Cancel" },
      confirmProps: { color: "red" },
      onConfirm: async () => {
        try {
          await deletePreset.mutateAsync(name);
          notifications.show({ color: "teal", message: `Deleted preset ${name}` });
        } catch (error) {
          notifications.show({
            color: "red",
            title: "Could not delete preset",
            message: error instanceof ApiError ? error.message : String(error),
          });
        }
      },
    });

  if (presets.error) return <ErrorState error={presets.error} />;
  if (presets.isLoading) return <LoadingState />;

  return (
    <Stack gap="sm">
      <Group justify="space-between">
        <Text size="sm" c="dimmed">
          {entries.length} preset{entries.length === 1 ? "" : "s"}
        </Text>
        <Button
          size="xs"
          leftSection={<IconPlus size={14} />}
          onClick={() => {
            setEditing(null);
            open();
          }}
        >
          New preset
        </Button>
      </Group>

      {entries.length === 0 ? (
        <EmptyState message="No presets yet. Presets let several streamers share one betting config." />
      ) : (
        <Stack gap="xs">
          {entries.map(([name, config]) => {
            const rules = config.prediction.strategy.detailed.detailed?.length ?? 0;
            const filters = config.prediction.filters.length;
            const used = usage.get(name) ?? 0;

            return (
              <Card key={name} padding="sm">
                <Group justify="space-between" wrap="nowrap" gap="xs">
                  <Stack gap={4} style={{ minWidth: 0 }}>
                    <Text fw={600} size="sm" truncate>
                      {name}
                    </Text>
                    <Group gap={6}>
                      <Badge size="xs" variant="light">
                        {rules} rule{rules === 1 ? "" : "s"}
                      </Badge>
                      <Badge size="xs" variant="light">
                        {filters} filter{filters === 1 ? "" : "s"}
                      </Badge>
                      <Badge size="xs" variant="light" color={used ? "violet" : "gray"}>
                        used by {used}
                      </Badge>
                    </Group>
                  </Stack>

                  <Group gap={4} wrap="nowrap">
                    <ActionIcon
                      variant="subtle"
                      aria-label={`Edit ${name}`}
                      onClick={() => {
                        setEditing({ name, config });
                        open();
                      }}
                    >
                      <IconPencil size={16} />
                    </ActionIcon>
                    <ActionIcon
                      variant="subtle"
                      color="red"
                      aria-label={`Delete ${name}`}
                      onClick={() => confirmDelete(name)}
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

      <PresetModal opened={opened} onClose={close} preset={editing} />
    </Stack>
  );
}
