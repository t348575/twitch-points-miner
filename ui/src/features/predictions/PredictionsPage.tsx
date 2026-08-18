import { useEffect, useMemo, useState } from "react";
import { Select, Stack, Title } from "@mantine/core";

import { useLiveStreamers } from "../../api/queries";
import { EmptyState, ErrorState, LoadingState } from "../../components/States";
import { PredictionCard } from "./PredictionCard";

export function PredictionsPage() {
  const live = useLiveStreamers();
  const [selected, setSelected] = useState<string | null>(null);

  // Most interesting streamers first: those with open predictions.
  const options = useMemo(() => {
    const streamers = live.data ?? [];
    return streamers
      .toSorted(
        (a, b) => Object.keys(b.state.predictions).length - Object.keys(a.state.predictions).length,
      )
      .map((s) => ({
        value: s.state.info.channelName,
        label: `${s.state.info.channelName} (${Object.keys(s.state.predictions).length})`,
      }));
  }, [live.data]);

  useEffect(() => {
    if (!selected && options.length) setSelected(options[0].value);
  }, [options, selected]);

  const current = useMemo(
    () => (live.data ?? []).find((s) => s.state.info.channelName === selected),
    [live.data, selected],
  );

  // `predictions` is a map of [event, betPlaced] tuples.
  const predictions = useMemo(() => Object.values(current?.state.predictions ?? {}), [current]);

  if (live.error) return <ErrorState error={live.error} title="Could not load live streamers" />;
  if (live.isLoading) return <LoadingState height={240} />;

  return (
    <Stack gap="md">
      <Title order={4}>Predictions</Title>

      {options.length === 0 ? (
        <EmptyState message="No streamers are live right now." />
      ) : (
        <>
          <Select
            label="Streamer"
            data={options}
            value={selected}
            onChange={setSelected}
            allowDeselect={false}
            searchable
          />

          {predictions.length === 0 ? (
            <EmptyState message="No open predictions for this streamer." />
          ) : (
            predictions.map(([event, placed]) => (
              <PredictionCard
                key={event.id}
                streamerName={current!.state.info.channelName}
                event={event}
                alreadyPlaced={placed}
              />
            ))
          )}
        </>
      )}
    </Stack>
  );
}
