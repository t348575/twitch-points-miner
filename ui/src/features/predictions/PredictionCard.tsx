import { useEffect, useMemo, useState } from "react";
import {
  Badge,
  Button,
  Card,
  Group,
  NumberInput,
  Progress,
  Radio,
  Stack,
  Text,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";

import { ApiError } from "../../api/http";
import { usePlaceBet } from "../../api/queries";
import { formatCountdown, formatPercent, formatPoints } from "../../lib/format";
import { outcomeColor } from "../../lib/colors";
import type { PredictionEvent } from "../../api/types";

interface PredictionCardProps {
  streamerName: string;
  event: PredictionEvent;
  /** The miner already placed a bet on this prediction. */
  alreadyPlaced: boolean;
}

/** Seconds left in the prediction window, ticking once per second. */
function useSecondsLeft(event: PredictionEvent): number {
  // Twitch timestamps carry an offset, so the default parser is correct here.
  const endsAt = useMemo(
    () => new Date(event.created_at).getTime() + event.prediction_window_seconds * 1000,
    [event.created_at, event.prediction_window_seconds],
  );

  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  return Math.max(0, Math.round((endsAt - now) / 1000));
}

export function PredictionCard({ streamerName, event, alreadyPlaced }: PredictionCardProps) {
  const placeBet = usePlaceBet();
  const secondsLeft = useSecondsLeft(event);
  const expired = secondsLeft <= 0;

  const [outcomeId, setOutcomeId] = useState<string | null>(null);
  const [points, setPoints] = useState<number | "">("");

  const totalPoints = event.outcomes.reduce((sum, o) => sum + o.total_points, 0);

  const submit = async () => {
    if (!outcomeId) return;
    try {
      const result = await placeBet.mutateAsync({
        streamer: streamerName,
        body: {
          event_id: event.id,
          outcome_id: outcomeId,
          // Blank means "let the configured strategy decide".
          points: points === "" ? null : Number(points),
        },
      });

      if (result === "placed") {
        notifications.show({ color: "teal", message: "Bet placed" });
      } else {
        notifications.show({
          color: "yellow",
          title: "No bet placed",
          message: "The configured strategy declined this prediction.",
        });
      }
    } catch (error) {
      notifications.show({
        color: "red",
        title: "Could not place bet",
        message: error instanceof ApiError ? error.message : String(error),
      });
    }
  };

  return (
    <Card>
      <Stack gap="sm">
        <Group justify="space-between" wrap="nowrap" align="flex-start">
          <Text fw={600}>{event.title}</Text>
          <Badge color={expired ? "gray" : secondsLeft < 30 ? "red" : "violet"} variant="light">
            {expired ? "Closed" : formatCountdown(secondsLeft)}
          </Badge>
        </Group>

        {alreadyPlaced && (
          <Badge color="teal" variant="light">
            Bet already placed
          </Badge>
        )}

        <Radio.Group value={outcomeId} onChange={setOutcomeId}>
          <Stack gap="xs">
            {event.outcomes.map((outcome, index) => {
              const share = totalPoints === 0 ? 0 : (outcome.total_points / totalPoints) * 100;
              return (
                <Card key={outcome.id} padding="xs">
                  <Stack gap={6}>
                    <Group justify="space-between" wrap="nowrap" gap="xs">
                      <Radio value={outcome.id} label={outcome.title} disabled={expired} />
                      <Text className="tpm-num" size="sm" fw={600}>
                        {formatPercent(share)}
                      </Text>
                    </Group>
                    {/* Colour is the only thing telling two outcome bars apart,
                        so each takes its position colour rather than the primary. */}
                    <Progress value={share} size="sm" color={outcomeColor(index)} />
                    <Group justify="space-between">
                      <Text className="tpm-num" size="xs" c="dimmed">
                        {formatPoints(outcome.total_points)} points
                      </Text>
                      <Text className="tpm-num" size="xs" c="dimmed">
                        {formatPoints(outcome.total_users)} users
                      </Text>
                    </Group>
                  </Stack>
                </Card>
              );
            })}
          </Stack>
        </Radio.Group>

        <Group align="flex-end" gap="sm" wrap="wrap">
          <NumberInput
            style={{ flex: 1, minWidth: 140 }}
            label="Points"
            description="Leave blank to use your strategy"
            placeholder="auto"
            min={1}
            allowDecimal={false}
            value={points}
            disabled={expired}
            onChange={(value) => setPoints(value === "" ? "" : Number(value))}
          />
          <Button disabled={expired || !outcomeId} loading={placeBet.isPending} onClick={submit}>
            Place bet
          </Button>
        </Group>
      </Stack>
    </Card>
  );
}
