<script lang="ts">
  import type { components, paths } from "./api";
  import createClient from "openapi-fetch";
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import { Calendar, RefreshCcw } from "lucide-svelte";
  import {
    CalendarDate,
    DateFormatter,
    type DateValue,
    getLocalTimeZone,
  } from "@internationalized/date";
  import { cn } from "$lib/utils.js";
  import { RangeCalendar } from "$lib/components/ui/range-calendar/index";
  import * as Popover from "$lib/components/ui/popover/index";
  import {
    VisXYContainer,
    VisLine,
    VisTooltip,
    VisCrosshair,
    VisBulletLegend,
    VisAxis,
  } from "@unovis/svelte";
  import { writable } from "svelte/store";
  import * as Select from "$lib/components/ui/select";
  import type { Selected } from "bits-ui";
  import * as Card from "$lib/components/ui/card";
  import * as Table from "$lib/components/ui/table";
  import { Input } from "$lib/components/ui/input";

  const client = createClient<paths>({
    baseUrl: import.meta.env.DEV
      ? "http://localhost:3000"
      : window.location.href,
  });

  let streamers: components["schemas"]["PubSub"]["streamers"];
  interface Streamer {
    name: string | undefined;
    id: number;
    points: number;
  }
  let streamers_name: Streamer[] = [];
  let selected_streamers: Streamer[] = [];
  let s_selected = streamers_name.map(() => "outline");
  let sort_selection = { value: "Descending", label: "Descending" };

  interface StreamerPrediction {
    label: string;
    value: number;
    streamer: components["schemas"]["LiveStreamer"];
  }
  let selected_streamer_for_prediction: StreamerPrediction | undefined;
  let live_streamers: StreamerPrediction[] = [];
  let prediction: (components["schemas"]["Event"] & boolean) | undefined;
  let pred_total_points = 0;
  let prediction_made: components["schemas"]["Prediction"] | null = null;
  let outcome: string | undefined;
  let prediction_points = 0;
  $: make_prediction_class = `flex flex-col justify-center items-center ${live_streamers.length == 0 ? "opacity-25 pointer-events-none" : ""}`;

  $: {
    s_selected = streamers_name.map((a) =>
      selected_streamers.find((b) => b.id == a.id) == undefined ? "" : "outline"
    );
    render_timeline();
  }

  const get_streamers = async (): Promise<components["schemas"]["PubSub"]> => {
    const { data, error } = await client.GET("/api");
    if (error) {
      throw error;
    }
    return data;
  };

  const get_timeline = async (
    from: string,
    to: string,
    channels: Streamer[]
  ): Promise<components["schemas"]["TimelineResult"][]> => {
    const { data, error } = await client.POST("/api/analytics/timeline", {
      body: {
        channels: channels.map((a) => a.id),
        from,
        to,
      },
    });

    if (error) {
      throw error;
    }
    return data;
  };

  const get_live_streamers = async (): Promise<
    components["schemas"]["LiveStreamer"][]
  > => {
    const { data, error } = await client.GET("/api/streamers/live");
    if (error) {
      throw error;
    }
    return data;
  };

  const get_last_prediction = async (
    channel_id: number,
    prediction_id: string
  ): Promise<components["schemas"]["Prediction"] | null> => {
    const { data, error } = await client.GET("/api/predictions/live", {
      params: {
        query: {
          prediction_id,
          channel_id,
        },
      },
    });
    if (error) {
      throw error;
    }
    return data;
  };

  interface PointData {
    idx: Date;
    value: components["schemas"]["TimelineResult"];
  }
  let timeline: PointData[] = [];
  let last_values: { id: number; value: number | undefined }[] = [];
  const x = (d: PointData) => d.idx;
  let y: ((d: PointData) => number | undefined)[] = [];
  const template = (d: PointData) => {
    let reason = "";
    switch (d.value.point.points_info) {
      case "FirstEntry": {
        reason = "Streamer added";
        break;
      }
      case "Watching": {
        reason = "Watching";
        break;
      }
      case "CommunityPointsClaimed": {
        reason = "Community bonus claim";
        break;
      }
      default: {
        reason = `Prediction - ${d.value.prediction?.title}`;
      }
    }
    return `Points: ${d.value.point.points_value}<br/>Reason: ${reason}<br/>At: ${new Date(d.value.point.created_at).toLocaleString()}`;
  };

  const df = new DateFormatter("en-UK", {
    dateStyle: "medium",
  });

  let prev = new Date();
  prev.setDate(prev.getDate() - 7);
  const now = new Date();

  let value = writable({
    start: new CalendarDate(
      prev.getFullYear(),
      prev.getMonth() + 1,
      prev.getDate()
    ),
    end: new CalendarDate(now.getFullYear(), now.getMonth() + 1, now.getDate()),
  });

  let currentDate: { start: CalendarDate; end: CalendarDate };
  value.subscribe((s) => {
    currentDate = s;
    render_timeline();
  });

  let startValue: DateValue | undefined = undefined;

  function sort_streamers(val: Selected<string>) {
    sort_selection = val;
    let v: boolean;
    if (val.value == "Descending") {
      v = true;
    } else {
      v = false;
    }

    streamers_name = streamers_name.sort((a, b) => {
      if (v) {
        return b.points - a.points;
      } else {
        return a.points - b.points;
      }
    });
  }

  onMount(async () => {
    streamers = (await get_streamers())["streamers"];
    for (const v in streamers) {
      streamers_name.push({
        name: streamers[v]?.info.channelName,
        id: parseInt(v, 10),
        points: streamers[v]?.points,
      });
    }
    streamers_name = streamers_name;
    sort_streamers(sort_selection);
    selected_streamers = [streamers_name[0]];

    await render_timeline();
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

  async function render_timeline() {
    let idx = 0;
    let from = new Date(
      currentDate?.start?.year,
      currentDate?.start?.month - 1,
      currentDate?.start?.day,
      0,
      0,
      0
    );
    let to = new Date(
      currentDate?.end?.year,
      currentDate?.end?.month - 1,
      currentDate?.end?.day,
      23,
      59,
      0
    );
    timeline = (
      await get_timeline(
        from.toISOString(),
        to.toISOString(),
        selected_streamers
      )
    ).map((a) => {
      idx++;
      return { idx: new Date(a.point.created_at), value: a };
    });

    last_values = selected_streamers.map((v) => ({
      id: v.id,
      value: timeline.findLast((a) => a.value.point.channel_id == v.id)?.value
        .point.points_value,
    }));

    y = selected_streamers.map((a) => {
      const copied_s: Streamer = JSON.parse(JSON.stringify(a));
      const lv = get_last_value(copied_s.id);
      return (d: PointData) =>
        copied_s.id == d.value.point.channel_id
          ? d.value.point.points_value
          : lv;
    });
  }

  function toggle_select(s: Streamer) {
    const before = selected_streamers.length;
    selected_streamers = selected_streamers.filter((a) => a.id != s.id);
    if (before == selected_streamers.length) {
      selected_streamers.push(s);
    }
  }

  function get_last_value(channel_id: number): number | undefined {
    for (const v of last_values) {
      if (v.id == channel_id) {
        return v.value;
      }
    }
  }

  async function select_streamer_for_prediction(
    v: Selected<number> | undefined
  ) {
    selected_streamer_for_prediction = v;

    if (v == undefined) {
      return;
    }

    const l = live_streamers.find((a) => v.value == a.value);
    if (l) {
      const preds = Object.keys(l.streamer.state.predictions);
      if (preds.length == 0) {
        prediction = undefined;
        prediction_made = null;
        return;
      }

      prediction = l.streamer.state.predictions[preds[0]][0];
      pred_total_points = prediction?.outcomes.reduce(
        (n, { total_points }) => (n += total_points),
        0
      );
      if (l.streamer.state.predictions[preds[0]][1]) {
        prediction_made = await get_last_prediction(
          v.value,
          l.streamer.state.predictions[preds[0]][0].id
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
    let event_id = prediction?.id;
    let outcome_id = outcome;

    let points = null;
    if (prediction_points > 0) {
      points = prediction_points;
    }

    await client.POST("/api/predictions/bet/{streamer}", {
      params: {
        path: {
          streamer: selected_streamer_for_prediction?.label,
        },
      },
      body: {
        event_id,
        outcome_id,
        points,
      },
    });

    prediction_points = 0;
  }
</script>

<div class="flex flex-col">
  <div class="flex flex-col">
    <div class="flex flex-row">
      <div class="flex- w-32">
        <Select.Root
          selected={sort_selection}
          onSelectedChange={(v) => sort_streamers(v)}
        >
          <Select.Trigger>
            <Select.Value placeholder="Points" />
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="Descending">Descending</Select.Item>
            <Select.Item value="Ascending">Ascending</Select.Item>
          </Select.Content>
        </Select.Root>
        {#each streamers_name as s, index}
          <Button
            variant={s_selected[index]}
            class="min-w-full my-2"
            on:click={() => toggle_select(s)}>{s.name}</Button
          >
        {/each}
      </div>
      <div class="flex-1 mx-10">
        <div class="flex flex-row m-0">
          <Popover.Root openFocus>
            <Popover.Trigger asChild let:builder>
              <Button
                variant="outline"
                class={cn(
                  "w-[300px] justify-start text-left font-normal",
                  !currentDate && "text-muted-foreground"
                )}
                builders={[builder]}
              >
                <Calendar class="mr-2 h-4 w-4" />
                {#if currentDate && currentDate.start}
                  {#if currentDate.end}
                    {df.format(currentDate.start.toDate(getLocalTimeZone()))} - {df.format(
                      currentDate.end.toDate(getLocalTimeZone())
                    )}
                  {:else}
                    {df.format(currentDate.start.toDate(getLocalTimeZone()))}
                  {/if}
                {:else if startValue}
                  {df.format(startValue.toDate(getLocalTimeZone()))}
                {:else}
                  Pick a date
                {/if}
              </Button>
            </Popover.Trigger>
            <Popover.Content class="w-auto p-0" align="start">
              <RangeCalendar
                bind:startValue
                placeholder={currentDate?.start}
                initialFocus
                numberOfMonths={1}
                onValueChange={(v) => value.set({ start: v.start, end: v.end })}
              />
            </Popover.Content>
          </Popover.Root>
        </div>
        <VisXYContainer data={timeline} class="mt-4">
          <VisTooltip />
          <VisLine {x} {y} curveType="linear" lineWidth={3}/>
          <VisAxis
            type="x"
            label="Time"
            tickFormat={(t) => new Date(t).toLocaleString()}
            gridLine={false}
            labelMargin={20}
          />
          <VisAxis type="y" label="Points" />
          <VisCrosshair {template} hideWhenFarFromPointer={false} {x} {y} />
        </VisXYContainer>
        <VisBulletLegend items={selected_streamers} />
      </div>
    </div>
  </div>
  <div class="w-1/2 self-center">
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
            onSelectedChange={(v) => select_streamer_for_prediction(v)}
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
                (a) => a.value == selected_streamer_for_prediction
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
                    class={outcome == o.id
                      ? "bg-zinc-700 hover:bg-zinc-700"
                      : ""}
                  >
                    <Table.Cell>{o.title}</Table.Cell>
                    <Table.Cell>{o.total_points}</Table.Cell>
                    <Table.Cell>{o.total_users}</Table.Cell>
                    <Table.Cell
                      >{(100.0 / (pred_total_points / o.total_points)).toFixed(
                        2
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
      {#if prediction && !prediction_made}
        <Card.Footer class="flex flex-col">
          {#if outcome}
            <p class="m-2">
              Outcome: {prediction.outcomes.find((a) => a.id == outcome)?.title}
            </p>
          {/if}
          <div class="flex flex-wrap justify-center">
            <Input
              type="number"
              placeholder="points"
              class="max-w-xs mr-2"
              bind:value={prediction_points}
              max={live_streamers.find(
                (a) => a.value == selected_streamer_for_prediction
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
        </Card.Footer>
      {:else if prediction && prediction_made}
        <Card.Footer class="flex flex-col">
          Bet placed! Outcome: {prediction.outcomes.find(
            (a) => a.id == prediction_made?.placed_bet.Some.outcome_id
          )?.title}
          Points: {prediction_made?.placed_bet.Some.points}
        </Card.Footer>
      {/if}
    </Card.Root>
  </div>
</div>
