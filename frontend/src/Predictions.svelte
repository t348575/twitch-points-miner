<script lang="ts">
  import * as Card from "$lib/components/ui/card";
  import * as Table from "$lib/components/ui/table";
  import * as Select from "$lib/components/ui/select";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import ErrorAlert from "$lib/components/ui/ErrorAlert.svelte";
  import { onMount } from "svelte";
  import {
    streamers,
    type Streamer,
    get_live_streamers,
    get_last_prediction,
    place_bet_streamer,
  } from "./common";
  import { get } from "svelte/store";
  import type { components } from "./api";
  import type { Selected } from "bits-ui";
  import { RefreshCcw } from "lucide-svelte";

  interface StreamerPrediction {
    label: string;
    value: number;
    streamer: components["schemas"]["LiveStreamer"];
  }

  type PredictionType = (components["schemas"]["Event"] & boolean) | undefined;

  let streamers_name: Streamer[] = [];
  let live_streamers: StreamerPrediction[] = [];
  let selected_streamer_for_prediction: StreamerPrediction | undefined;
  let pred_total_points = 0;
  let prediction_made: components["schemas"]["Prediction"] | null = null;
  let prediction: PredictionType;
  let outcome: string | undefined;
  let prediction_points: undefined | string = undefined;
  let error_message: undefined | string = undefined;
  let prediction_time_up = false;

  $: make_prediction_class = `flex flex-col justify-center items-center ${live_streamers.length == 0 ? "opacity-25 pointer-events-none" : ""}`;

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
      await select_streamer_for_prediction(selected_streamer_for_prediction);
    }
  }

  async function select_streamer_for_prediction(
    v: Selected<number> | undefined,
  ) {
    selected_streamer_for_prediction = v as StreamerPrediction;

    if (v == undefined) {
      return;
    }

    const l = live_streamers.find((a) => v.value == a.value);
    if (l) {
      const preds: string[] = Object.keys(l.streamer.state.predictions);
      if (preds.length == 0) {
        prediction = undefined;
        prediction_made = null;
        return;
      }

      const preds_list = l.streamer.state.predictions[
        preds[0] as string
      ] as PredictionType[];
      prediction = preds_list[0];

      prediction_time_up = new Date(new Date(prediction?.created_at as string).getTime() + prediction?.prediction_window_seconds * 1000) > new Date();
      pred_total_points =
        prediction?.outcomes.reduce(
          (n, { total_points }) => (n += total_points),
          0,
        ) || pred_total_points;
      if (preds_list[1]) {
        prediction_made = await get_last_prediction(
          v.value,
          preds_list[0]?.id as string,
        );
      }
    }
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
          (a) => a.value == selected_streamer_for_prediction?.value,
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
      <Card.Content class={make_prediction_class}>
        <div class="w-1/2 flex flex-row">
          <Select.Root
            selected={selected_streamer_for_prediction}
            onSelectedChange={select_streamer_for_prediction}
          >
            <Select.Trigger>
              <Select.Value placeholder="Streamer" />
            </Select.Trigger>
            <Select.Content>
              {#each live_streamers as s}
                <Select.Item value={s.value}>{s.label}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
          <Button
            on:click={refresh_live_streamers}
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
              Points: {live_streamers.find(
                (a) => a.value == selected_streamer_for_prediction?.value,
              )?.streamer.state.points}
            </p>
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
                    on:click={() => choose_outcome(o.id)}
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
        {#if prediction && prediction_made == null || (prediction_made !== null && typeof prediction_made.placed_bet == 'string')}
          {#if outcome}
            <p class="m-2">
              Selected outcome: {prediction.outcomes.find((a) => a.id == outcome)
                ?.title}
            </p>
          {/if}
          <div class="flex flex-wrap justify-center">
            {#if error_message}
              <ErrorAlert message={error_message} />
            {/if}
            <Input
              type="number"
              placeholder="points"
              class="max-w-48 mr-2"
              bind:value={prediction_points}
              max={live_streamers.find(
                (a) => a.value == selected_streamer_for_prediction?.value,
              )?.streamer.state.points}
              min={0}
            />
            <Button on:click={place_bet} disabled={outcome == undefined}
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