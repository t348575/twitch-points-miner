import { useEffect, useMemo, useState } from "react";
import {
  Alert,
  Button,
  Card,
  Group,
  NumberInput,
  Select,
  Stack,
  Switch,
  Text,
  Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconInfoCircle, IconPlus } from "@tabler/icons-react";

import { ApiError } from "../../api/http";
import {
  useAppState,
  useMaxWatching,
  useSetMaxWatching,
  useSetWatchPriority,
  useSetWatchStreak,
  useWatchPriority,
  useWatchStreak,
} from "../../api/queries";
import { ErrorState, LoadingState } from "../../components/States";
import { WatchPriorityList } from "./WatchPriorityList";

function reportError(title: string, error: unknown) {
  notifications.show({
    color: "red",
    title,
    message: error instanceof ApiError ? error.message : String(error),
  });
}

export function WatchingTab() {
  const appState = useAppState();
  const maxWatching = useMaxWatching();
  const watchStreak = useWatchStreak();
  const watchPriority = useWatchPriority();

  const setMaxWatching = useSetMaxWatching();
  const setWatchStreak = useSetWatchStreak();
  const setWatchPriority = useSetWatchPriority();

  const [maxValue, setMaxValue] = useState<number>(2);
  const [priority, setPriority] = useState<string[]>([]);
  const [toAdd, setToAdd] = useState<string | null>(null);

  useEffect(() => {
    if (maxWatching.data !== undefined) setMaxValue(maxWatching.data);
  }, [maxWatching.data]);

  useEffect(() => {
    if (watchPriority.data) setPriority(watchPriority.data);
  }, [watchPriority.data]);

  const knownNames = useMemo(
    () =>
      Object.values(appState.data?.streamers ?? {})
        .map((s) => s.info.channelName)
        .toSorted((a, b) => a.localeCompare(b)),
    [appState.data],
  );

  // The backend rejects any name that is not a configured streamer, and a stale
  // list would make the next save fail, so drop unknown entries as they appear.
  const validPriority = useMemo(
    () => priority.filter((name) => knownNames.includes(name)),
    [priority, knownNames],
  );

  const addable = useMemo(
    () => knownNames.filter((name) => !validPriority.includes(name)),
    [knownNames, validPriority],
  );

  const priorityDirty = JSON.stringify(validPriority) !== JSON.stringify(watchPriority.data ?? []);

  if (maxWatching.error) return <ErrorState error={maxWatching.error} />;
  if (maxWatching.isLoading || watchPriority.isLoading) return <LoadingState />;

  return (
    <Stack gap="md">
      <Card>
        <Stack gap="sm">
          <Title order={5}>Concurrent streams</Title>
          <Text size="sm" c="dimmed">
            How many streams to watch at once for viewership points. Set 0 to pause watching.
          </Text>
          <Group align="flex-end" gap="sm">
            <NumberInput
              style={{ width: 120 }}
              min={0}
              max={10}
              allowDecimal={false}
              value={maxValue}
              onChange={(value) => setMaxValue(Number(value) || 0)}
            />
            <Button
              size="sm"
              disabled={maxValue === maxWatching.data}
              loading={setMaxWatching.isPending}
              onClick={async () => {
                try {
                  await setMaxWatching.mutateAsync(maxValue);
                  notifications.show({ color: "teal", message: "Updated concurrent streams" });
                } catch (error) {
                  reportError("Could not update", error);
                }
              }}
            >
              Save
            </Button>
          </Group>
        </Stack>
      </Card>

      <Card>
        <Stack gap="sm">
          <Title order={5}>Watch streak</Title>
          <Switch
            label="Prioritize newly live channels briefly"
            description="Raises the chance of receiving WATCH_STREAK point events"
            checked={watchStreak.data ?? true}
            disabled={watchStreak.isLoading || setWatchStreak.isPending}
            onChange={async (event) => {
              const next = event.currentTarget.checked;
              try {
                await setWatchStreak.mutateAsync(next);
                notifications.show({
                  color: "teal",
                  message: next ? "Watch streak enabled" : "Watch streak disabled",
                });
              } catch (error) {
                reportError("Could not update watch streak", error);
              }
            }}
          />
        </Stack>
      </Card>

      <Card>
        <Stack gap="sm">
          <Title order={5}>Watch priority</Title>
          <Text size="sm" c="dimmed">
            When more channels are live than can be watched, those higher in this list win. Channels
            not listed fall back to their order in the config.
          </Text>

          {validPriority.length === 0 ? (
            <Alert color="gray" icon={<IconInfoCircle size={16} />}>
              No priority set. Live channels are picked in config order.
            </Alert>
          ) : (
            <WatchPriorityList items={validPriority} onChange={setPriority} />
          )}

          <Group gap="xs" align="flex-end">
            <Select
              style={{ flex: 1 }}
              placeholder={addable.length ? "Add a channel" : "All channels are listed"}
              data={addable}
              value={toAdd}
              disabled={addable.length === 0}
              searchable
              onChange={setToAdd}
            />
            <Button
              size="sm"
              variant="light"
              leftSection={<IconPlus size={14} />}
              disabled={!toAdd}
              onClick={() => {
                if (!toAdd) return;
                setPriority([...validPriority, toAdd]);
                setToAdd(null);
              }}
            >
              Add
            </Button>
          </Group>

          <Group justify="flex-end">
            <Button
              size="sm"
              disabled={!priorityDirty}
              loading={setWatchPriority.isPending}
              onClick={async () => {
                try {
                  await setWatchPriority.mutateAsync(validPriority);
                  notifications.show({ color: "teal", message: "Saved watch priority" });
                } catch (error) {
                  reportError("Could not save watch priority", error);
                }
              }}
            >
              Save order
            </Button>
          </Group>
        </Stack>
      </Card>
    </Stack>
  );
}
