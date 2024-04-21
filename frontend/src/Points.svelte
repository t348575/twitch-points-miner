<script lang="ts">
  import type { components } from "./api";
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
  import { get_timeline, streamers, type Streamer } from "./common";
  let margin = {top: 50};

  let streamers_name: Streamer[] = [];
  let selected_streamers: Streamer[] = [];
  let s_selected = streamers_name.map(() => "outline");
  let sort_selection: Selected<string> = {
    value: "Descending",
    label: "Descending",
  };

  $: {
    if (streamers_name) {
      s_selected = streamers_name.map((a) =>
        selected_streamers.find((b) => b.id == a.id) == undefined
          ? ""
          : "outline",
      );
      render_timeline();
    }
  }

  streamers.subscribe(s => {
    streamers_name = s
    if (streamers_name.length > 0) {
      sort_streamers(sort_selection);
      selected_streamers = [streamers_name[0] as Streamer];

      render_timeline();
    }
  })

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
      prev.getDate(),
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

  async function render_timeline() {
    let idx = 0;
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
</script>

<div class="flex flex-col">
  <div class="flex flex-col">
    <div class="flex flex-row">
      <div class="flex- w-32">
        <Select.Root
          selected={sort_selection}
          onSelectedChange={sort_streamers}
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
        <VisXYContainer data={timeline} class="mt-4" {margin} height={500}>
          <VisTooltip horizontalShift={50} verticalShift={50} verticalPlacement="top" />
          <VisLine {x} {y} curveType="linear" lineWidth={3} />
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
</div>
