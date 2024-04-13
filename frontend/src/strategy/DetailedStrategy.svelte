<script lang="ts">
  import { Input } from "$lib/components/ui/input";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import { Separator } from "$lib/components/ui/separator";
  import { X, Plus, CircleAlert } from "lucide-svelte";
  import type { components } from "src/api";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import {
    default_parse_strategy,
    default_validate_strategy,
    type ValidateStrategy,
  } from "../common";

  let error: undefined | string = undefined;
  export let high_odds: {
    data: components["schemas"]["HighOdds"];
    error: string | undefined;
  }[] = [];
  export let default_odds: components["schemas"]["DefaultPrediction"] = {
    points: {
      // @ts-ignore
      max_value: undefined,
      // @ts-ignore
      percent: undefined,
    },
    max_percentage: undefined,
    min_percentage: undefined,
  };

  function default_high_odds(): components["schemas"]["HighOdds"] {
    return {
      high_odds_attempt_rate: undefined,
      high_odds_points: {
        // @ts-ignore
        max_value: undefined,
        // @ts-ignore
        percent: undefined,
      },
      high_threshold: undefined,
      low_threshold: undefined,
    };
  }

  export function validate(): ValidateStrategy {
    let status = true;
    error = undefined;
    for (let v of high_odds) {
      v.error = default_validate_strategy(v.data);
      status = v.error ? false : true;
    }

    error = default_validate_strategy(default_odds);
    if (status) {
      status = error ? false : true;
    }

    const high_odds_items = [];
    let default_odds_items;
    if (status) {
      for (const v of high_odds) {
        high_odds_items.push(default_parse_strategy(v.data));
      }
      default_odds_items = default_parse_strategy(default_odds);
    }

    high_odds = high_odds;
    // @ts-ignore
    return {
      status,
      data: {
        detailed: {
          default: default_odds_items,
          high_odds: high_odds_items,
        },
      },
    };
  }
</script>

<div class="flex flex-col">
  {#if error}
    <Alert.Root class="border-red-500 mb-4">
      <CircleAlert class="h-4 w-4" />
      <Alert.Title>Error</Alert.Title>
      <Alert.Description>{error}</Alert.Description>
    </Alert.Root>
  {/if}
  <p class="text-center">Default</p>
  <div class="grid grid-rows-2 grid-cols-11 gap-1 items-center">
    <p class="col-span-3">Threshold</p>
    <Input
      type="number"
      placeholder="Max percentage"
      class="col-span-4"
      bind:value={default_odds.max_percentage}
    />
    <Input
      type="number"
      placeholder="Min percentage"
      class="col-span-4"
      bind:value={default_odds.min_percentage}
    />

    <p class="col-span-3">Points</p>
    <Input
      type="number"
      placeholder="Max value"
      class="col-span-4"
      bind:value={default_odds.points.max_value}
    />
    <Input
      type="number"
      placeholder="Percentage"
      class="col-span-4"
      bind:value={default_odds.points.percent}
    />
  </div>
  <div class="flex m-4 items-center justify-center">
    High odds
    <Button
      variant="outline"
      class="rounded-full w-10 h-10 p-0 ml-4"
      on:click={() =>
        (high_odds = [
          ...high_odds,
          { data: default_high_odds(), error: undefined },
        ])}
    >
      <Plus class="rounded-full w-10 h-10" size={4} />
    </Button>
  </div>
  <ScrollArea class="odds-scroll-area">
    {#each high_odds as f, index}
      {#if f.error}
        <Alert.Root class="border-red-500 mb-4">
          <CircleAlert class="h-4 w-4" />
          <Alert.Title>Error</Alert.Title>
          <Alert.Description>{f.error}</Alert.Description>
        </Alert.Root>
      {/if}
      <div class="grid grid-rows-3 grid-cols-12 gap-1 items-center my-4 mx-1">
        <Input
          type="number"
          placeholder="Low threshold"
          class="col-span-5"
          bind:value={f.data.low_threshold}
        />
        <Input
          type="number"
          placeholder="High threshold"
          class="col-span-5"
          bind:value={f.data.high_threshold}
        />
        <Button
          variant="outline"
          class="rounded-full w-10 h-10 p-0 row-span-2 col-span-2 place-self-center"
          on:click={() => {
            high_odds.splice(index, 1);
            high_odds = high_odds;
          }}
        >
          <X class="rounded-full w-10 h-10" size={4} />
        </Button>
        <Input
          type="number"
          placeholder="Attempt rate"
          class="col-span-10"
          bind:value={f.data.high_odds_attempt_rate}
        />
        <Input
          type="number"
          placeholder="Max value"
          class="col-span-5"
          bind:value={f.data.high_odds_points.max_value}
        />
        <Input
          type="number"
          placeholder="Percentage"
          class="col-span-5"
          bind:value={f.data.high_odds_points.percent}
        />
        <p class="col-span-2 text-center">Points</p>
      </div>
      {#if index + 1 != high_odds.length}
        <Separator />
      {/if}
    {/each}
  </ScrollArea>
</div>
