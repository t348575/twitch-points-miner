<script lang="ts">
  import { Input } from "$lib/components/ui/input";
  import * as Select from "$lib/components/ui/select";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import { Separator } from "$lib/components/ui/separator";
  import { X, Plus, CircleAlert } from "lucide-svelte";
  import type { components } from "src/api";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import { type ValidateStrategy } from "../common";
  import {
    DETAILED_STRATEGY_ODDS_COMPARISON_TYPES,
    detailed_strategy_parse,
    validate_detailed_strategy,
  } from "./strategy";

  let error: undefined | string = undefined;
  export let detailed_odds: {
    data: components["schemas"]["DetailedOdds"];
    _type: { value: string; label: string };
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

  function default_high_odds(): components["schemas"]["DetailedOdds"] {
    return {
      _type: "Le",
      attempt_rate: undefined,
      points: {
        // @ts-ignore
        max_value: undefined,
        // @ts-ignore
        percent: undefined,
      },
      threshold: undefined,
    };
  }

  export function validate(): ValidateStrategy {
    let status = true;
    error = undefined;
    for (let v of detailed_odds) {
      v.error = validate_detailed_strategy(v.data);
      status = v.error ? false : true;
    }

    error = validate_detailed_strategy(default_odds);
    if (status) {
      status = error ? false : true;
    }

    const detailed_items = [];
    let default_odds_items;
    if (status) {
      for (const v of detailed_odds) {
        v.data._type = v._type.value;
        detailed_items.push(detailed_strategy_parse(v.data));
      }
      default_odds_items = detailed_strategy_parse(default_odds);
    }

    detailed_odds = detailed_odds;
    // @ts-ignore
    return {
      status,
      data: {
        detailed: {
          default: default_odds_items,
          detailed: detailed_items,
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
    Detailed odds
    <Button
      variant="outline"
      class="rounded-full w-10 h-10 p-0 ml-4"
      on:click={() =>
        (detailed_odds = [
          ...detailed_odds,
          {
            data: default_high_odds(),
            error: undefined,
            _type: DETAILED_STRATEGY_ODDS_COMPARISON_TYPES[0],
          },
        ])}
    >
      <Plus class="rounded-full w-10 h-10" size={4} />
    </Button>
  </div>
  <ScrollArea class="odds-scroll-area flex flex-col">
    {#each detailed_odds as f, index}
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
          placeholder="Threshold"
          class="col-span-5"
          bind:value={f.data.threshold}
        />
        <Select.Root bind:selected={f._type}>
          <Select.Trigger class="col-span-5">
            <Select.Value placeholder="Type" />
          </Select.Trigger>
          <Select.Content>
            {#each DETAILED_STRATEGY_ODDS_COMPARISON_TYPES as d}
              <Select.Item value={d.value}>{d.label}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
        <Button
          variant="outline"
          class="rounded-full w-10 h-10 p-0 row-span-2 col-span-2 place-self-center"
          on:click={() => {
            detailed_odds.splice(index, 1);
            detailed_odds = detailed_odds;
          }}
        >
          <X class="rounded-full w-10 h-10" size={4} />
        </Button>
        <Input
          type="number"
          placeholder="Attempt rate"
          class="col-span-10"
          bind:value={f.data.attempt_rate}
        />
        <Input
          type="number"
          placeholder="Max value"
          class="col-span-5"
          bind:value={f.data.points.max_value}
        />
        <Input
          type="number"
          placeholder="Percentage"
          class="col-span-5"
          bind:value={f.data.points.percent}
        />
        <p class="col-span-2 text-center">Points</p>
      </div>
      {#if index + 1 != detailed_odds.length}
        <Separator />
      {/if}
    {/each}
  </ScrollArea>
</div>
