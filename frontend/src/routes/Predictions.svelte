<script lang="ts">
  import * as Card from "$lib/components/ui/card";
  import * as Table from "$lib/components/ui/table";
  import * as Select from "$lib/components/ui/select";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import ErrorAlert from "$lib/components/ui/ErrorAlert.svelte";
  import { onDestroy, onMount } from "svelte";
  import {
    streamers,
    type Streamer,
    get_live_streamers,
    get_last_prediction,
    place_bet_streamer,
  } from "../common";
  import { get } from "svelte/store";
  import type { components } from "../api";
  import { RefreshCcw } from "lucide-svelte";

  interface StreamerPrediction {
    label: string;
    value: number;
    streamer: components["schemas"]["LiveStreamer"];
  }

  type PredictionType = (components["schemas"]["Event"] & boolean) | undefined;

  let streamers_name: Streamer[] = [];
  let live_streamers: StreamerPrediction[] = $state([]);
  let selected_streamer_for_prediction: number | undefined = $state();
  let pred_total_points = $state(0);
  let prediction_made: components["schemas"]["Prediction"] | null = $state(null);
  let prediction: PredictionType = $state();
  let outcome: string | undefined = $state();
  let prediction_points: undefined | string = $state();
  let error_message: undefined | string = $state();
  let prediction_time_up = $state(false);
  let time_left = $state(0);
  let interval: number | undefined;

  $effect(() => {
    if (!prediction) {
      if (interval) clearInterval(interval);
      time_left = 0;
      return;
    }

    if (interval) clearInterval(interval);

    const tick = () => {
      const deadline =
        new Date(prediction.created_at).getTime() +
        prediction.prediction_window_seconds * 1000;

      time_left = Math.max(0, Math.floor((deadline - Date.now()) / 1000));
    };

    tick();
    interval = setInterval(tick, 1000) as unknown as number;

    return () => {
      if (interval) clearInterval(interval);
    };
  });

  onDestroy(() => {
    if (interval) {
      clearInterval(interval)
    }
  })

  const make_prediction_class = $derived(() => `flex flex-col justify-center items-center ${live_streamers.length === 0 ? "opacity-25 pointer-events-none" : ""}`);

  onMount(async () => {
    streamers_name = get(streamers);
    await refresh_live_streamers();
  });

  async function refresh_live_streamers() {
    const live_s = await get_live_streamers();
    live_streamers = live_s
      .sort((a, b) => {
        return (
          Object.keys(b.state.predictions).length -
          Object.keys(a.state.predictions).length
        );
      })
      .map((a) => ({
        value: a.id,
        label: a.state.info.channelName,
        streamer: a,
      }));

    if (selected_streamer_for_prediction) {
      select_streamer_for_prediction();
    }
  }

  const trigger_selected_streamer = $derived((() => {
    if (!selected_streamer_for_prediction) return "Select streamer";
    return (
      live_streamers.find(a => a.value === selected_streamer_for_prediction)?.label ??
      "Select streamer"
    );
  })());

  $effect(select_streamer_for_prediction);

  function select_streamer_for_prediction() {
    if (!selected_streamer_for_prediction) return;

    let cancelled = false;
    (async (id: number) => {
      prediction = undefined;
      prediction_made = null;

      const l = live_streamers.find(a => a.value === id);
      if (!l) return;

      const preds = Object.keys(l.streamer.state.predictions);
      if (!preds.length) return;

      const list = l.streamer.state.predictions[preds[0] as string] as PredictionType[];
      const first = list[0];
      if (!first || cancelled) return;

      prediction = first;

      const deadline =
        new Date(first.created_at).getTime() + first.prediction_window_seconds * 1000;
      prediction_time_up = deadline - Date.now() < 0;

      pred_total_points =
        first.outcomes.reduce((n, { total_points }) => n + total_points, 0);

      if (list[1]) {
        const last = await get_last_prediction(id, first.id as string);
        if (!cancelled) prediction_made = last;
      }
    })(selected_streamer_for_prediction);

    return () => { cancelled = true; };
  }

  function choose_outcome(id: string | undefined) {
    if (outcome == id) {
      outcome = undefined;
    } else {
      outcome = id;
    }
  }

  async function place_bet() {
    let event_id: string = prediction?.id as string;
    let outcome_id = outcome as string;

    let points = null;
    if (prediction_points != undefined) {
      points = parseInt(prediction_points, 10);
    }

    try {
      await place_bet_streamer(
        live_streamers.find(
          (a) => a.value == selected_streamer_for_prediction,
        )?.streamer.state.info.channelName as string,
        event_id,
        outcome_id,
        points,
      );
    } catch (err) {
      error_message = err as string;
      return;
    }

    error_message = undefined;
    prediction_points = undefined;
    streamers_name = get(streamers);
    await refresh_live_streamers();
  }
</script>

<div class="flex flex-col">
  <div class="w-1/2 self-center mb-4">
    <Card.Root>
      <Card.Header class="flex flex-row justify-center items-center">
        <p class="text-xl mr-2 inline">
          {live_streamers.length == 0 ? "No streamers live" : "Make prediction"}
        </p>
      </Card.Header>
      <Card.Content class={make_prediction_class()}>
        <div class="w-1/2 flex flex-row">
          <Select.Root type="single" bind:value={selected_streamer_for_prediction}>
            <Select.Trigger class="w-full">{trigger_selected_streamer}</Select.Trigger>
            <Select.Content>
              {#each live_streamers as s}
                <Select.Item value={s.value} label={s.label} class="rounded-sm px-2 py-1.5 text-sm data-[highlighted]:bg-muted data-[highlighted]:text-foreground hover:bg-muted"></Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>

          <Button
            onclick={refresh_live_streamers}
            variant="outline"
            size="icon"
            class="self-center ml-2"
          >
            <RefreshCcw
              class="h-[1.2rem] w-[1.2rem] rotate-0 scale-100 transition-all"
            />
          </Button>
        </div>
        <div class="w-3/4 mt-8 text-center">
          {#if prediction}
            <p class="text-xl">
              Channel points: {live_streamers.find(
                (a) => a.value == selected_streamer_for_prediction,
              )?.streamer.state.points}
            </p>
            {#if time_left > 0}
              <p class="text-lg">Time left: {time_left}s</p>
            {/if}
            <p class="text-xl">{prediction.title}</p>
            <Table.Root>
              <Table.Header>
                <Table.Row>
                  <Table.Head class="text-center">Outcome</Table.Head>
                  <Table.Head class="text-center">Points</Table.Head>
                  <Table.Head class="text-center">Users</Table.Head>
                  <Table.Head class="text-center">Odds</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each prediction.outcomes as o}
                  <Table.Row
                    onclick={() => choose_outcome(o.id)}
                    class={outcome == o.id ? "bg-zinc-700 hover:bg-zinc-700" : ""}
                  >
                    <Table.Cell>{o.title}</Table.Cell>
                    <Table.Cell>{o.total_points}</Table.Cell>
                    <Table.Cell>{o.total_users}</Table.Cell>
                    <Table.Cell
                      >{(100.0 / (pred_total_points / o.total_points)).toFixed(
                        2,
                      )}</Table.Cell
                    >
                  </Table.Row>
                {/each}
              </Table.Body>
            </Table.Root>
          {:else if selected_streamer_for_prediction != undefined && prediction == undefined}
            <p>No prediction in progress</p>
          {/if}
        </div>
      </Card.Content>
      <Card.Footer class="flex flex-col">
        {#if prediction && (prediction_made == null || (prediction_made !== null && typeof prediction_made.placed_bet == 'string')) && !prediction_time_up}
          {#if outcome}
            <p class="m-2">
              Selected outcome: {prediction.outcomes.find((a) => a.id == outcome)
                ?.title}
            </p>
          {/if}
          <div class="flex flex-wrap justify-center">
            {#if error_message}
              <ErrorAlert content={error_message} />
            {/if}
            <Input
              type="number"
              placeholder="points"
              class="max-w-48 mr-2"
              bind:value={prediction_points}
              max={live_streamers.find(
                (a) => a.value == selected_streamer_for_prediction,
              )?.streamer.state.points}
              min={0}
            />
            <Button onclick={place_bet} disabled={outcome == undefined}
              >Make prediction</Button
            >
            <p class="mt-2">
              Note: If prediction points is 0, the app runs the prediction logic
              without the filters
            </p>
          </div>
        {:else if prediction && prediction_made && typeof prediction_made.placed_bet !== 'string'}
          Bet placed! Outcome: {prediction.outcomes.find(
            (a) => a.id == prediction_made?.placed_bet.Some.outcome_id,
          )?.title}
        Points: {prediction_made?.placed_bet.Some.points}
        {:else if prediction && prediction_time_up && prediction_made == null }
          Prediction time up
        {/if}
      </Card.Footer>
    </Card.Root>
  </div>  
</div>