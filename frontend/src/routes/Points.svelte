<script lang="ts">
  import type { components } from "../api";
  import { Button } from "$lib/components/ui/button";
  import { Calendar } from "lucide-svelte";
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
  import { get_timeline, streamers, type Streamer } from "../common";
  let margin = { top: 50 };

  let streamers_name: Streamer[] = [];
  let selected_streamers: Streamer[] = [];
  let s_selected = streamers_name.map(() => "outline");
  let sort_selection: Selected<string> = {
    value: "Descending",
    label: "Descending",
  };

  // Persistent color map for consistent streamer colors
  const COLORS = [
    "#6366f1",
    "#f43f5e",
    "#10b981",
    "#f59e0b",
    "#8b5cf6",
    "#ec4899",
    "#14b8a6",
    "#f97316",
    "#3b82f6",
    "#84cc16",
  ];
  const streamerColorMap = new Map<number, string>();

  function getStreamerColor(streamerId: number): string {
    if (!streamerColorMap.has(streamerId)) {
      const colorIndex = streamerColorMap.size % COLORS.length;
      streamerColorMap.set(streamerId, COLORS[colorIndex]);
    }
    return streamerColorMap.get(streamerId)!;
  }

  streamers.subscribe((s) => {
    streamers_name = s;
    if (streamers_name.length > 0) {
      sort_streamers(sort_selection);
      selected_streamers = streamers_name.slice(0, 10) as Streamer[];
    }
  });

  interface PointData {
    idx: Date;
    value: components["schemas"]["TimelineResult"];
  }

  interface GroupedStreamerData {
    streamerId: number;
    streamerName: string;
    color: string;
    data: PointData[];
  }

  let timeline: PointData[] = [];
  let groupedData: GroupedStreamerData[] = [];
  let last_values: { id: number; value: number | undefined }[] = [];
  const x = (d: PointData) => d.idx;
  const y = (d: PointData) => d.value.point.points_value;
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

    let difference = "";
    if (d.value.difference !== null && d.value.difference !== 0) {
      if (d.value.difference > 0) {
        difference = `(+${d.value.difference})`;
      } else {
        difference = `(${d.value.difference})`;
      }
    }

    return `<b>${d.value.point.channel_id}</b><br/>Points: ${difference} ${d.value.point.points_value}<br/>Reason: ${reason}<br/>At: ${new Date(d.value.point.created_at).toLocaleString()}`;
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
      prev.getDate(),
    ),
    end: new CalendarDate(now.getFullYear(), now.getMonth() + 1, now.getDate()),
  });

  let currentDate: { start: CalendarDate; end: CalendarDate };

  $: {
    if (streamers_name || $value) {
      currentDate = $value;
      s_selected = streamers_name.map((a) =>
        selected_streamers.find((b) => b.id == a.id) == undefined
          ? ""
          : "outline",
      );
      render_timeline();
    }
  }

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

  async function render_timeline() {
    if (selected_streamers.length === 0) {
      groupedData = [];
      return;
    }

    let from = new Date(
      currentDate?.start?.year,
      currentDate?.start?.month - 1,
      currentDate?.start?.day,
      0,
      0,
      0,
    );
    let to = new Date(
      currentDate?.end?.year,
      currentDate?.end?.month - 1,
      currentDate?.end?.day,
      23,
      59,
      0,
    );
    timeline = (
      await get_timeline(
        from.toISOString(),
        to.toISOString(),
        selected_streamers,
      )
    ).map((a) => {
      return { idx: new Date(a.point.created_at), value: a };
    });

    last_values = selected_streamers.map((v) => ({
      id: v.id,
      value: timeline.findLast((a) => a.value.point.channel_id == v.id)?.value
        .point.points_value,
    }));

    // Group data by streamer to prevent line interleaving
    const dataByStreamer = new Map<number, PointData[]>();
    for (const point of timeline) {
      const streamerId = point.value.point.channel_id;
      if (!dataByStreamer.has(streamerId)) {
        dataByStreamer.set(streamerId, []);
      }
      dataByStreamer.get(streamerId)!.push(point);
    }

    // Build grouped data array with colors
    groupedData = selected_streamers.map((s) => ({
      streamerId: s.id,
      streamerName: s.name,
      color: getStreamerColor(s.id),
      data: dataByStreamer.get(s.id) ?? [],
    }));
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
</script>

<div class="flex flex-col">
  <div class="flex flex-col">
    <div class="flex flex-col md:flex-row gap-4">
      <div class="w-full md:w-32 flex flex-col gap-2">
        <Select.Root
          selected={sort_selection}
          onSelectedChange={sort_streamers}
        >
          <Select.Trigger class="w-full">
            <Select.Value placeholder="Points" />
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="Descending">Descending</Select.Item>
            <Select.Item value="Ascending">Ascending</Select.Item>
          </Select.Content>
        </Select.Root>
        <div class="flex flex-wrap md:flex-col gap-2 md:gap-0">
          {#each streamers_name as s, index}
            <Button
              variant={s_selected[index]}
              class="flex-1 md:w-full md:my-2"
              on:click={() => toggle_select(s)}>{s.name}</Button
            >
          {/each}
        </div>
      </div>
      <div class="flex-1 mx-0 md:mx-10 overflow-hidden">
        <div class="flex flex-row m-0">
          <Popover.Root openFocus>
            <Popover.Trigger asChild let:builder>
              <Button
                variant="outline"
                class={cn(
                  "w-full max-w-[300px] justify-start text-left font-normal",
                  !currentDate && "text-muted-foreground",
                )}
                builders={[builder]}
              >
                <Calendar class="mr-2 h-4 w-4" />
                {#if currentDate && currentDate.start}
                  {#if currentDate.end}
                    {df.format(currentDate.start.toDate(getLocalTimeZone()))} - {df.format(
                      currentDate.end.toDate(getLocalTimeZone()),
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
        <VisXYContainer class="mt-4" {margin} height={500}>
          <VisTooltip
            horizontalShift={50}
            verticalShift={50}
            verticalPlacement="top"
          />
          {#each groupedData as group (group.streamerId)}
            <VisLine
              data={group.data}
              {x}
              {y}
              curveType="linear"
              lineWidth={1}
              color={group.color}
            />
          {/each}
          <VisAxis
            type="x"
            label="Time"
            tickFormat={(t) => new Date(t).toLocaleString()}
            gridLine={false}
            labelMargin={20}
            data={timeline}
          />
          <VisAxis type="y" label="Points" data={timeline} />
          <VisCrosshair
            {template}
            hideWhenFarFromPointer={true}
            {x}
            {y}
            data={timeline}
          />
        </VisXYContainer>
        <VisBulletLegend
          items={groupedData.map((g) => ({
            name: g.streamerName,
            color: g.color,
          }))}
        />
      </div>
    </div>
  </div>
</div>
