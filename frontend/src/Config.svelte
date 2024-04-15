<script lang="ts">
  import * as Select from "$lib/components/ui/select";
  import ErrorAlert from "$lib/components/ui/ErrorAlert.svelte";
  import { Input } from "$lib/components/ui/input";
  import { Button } from "$lib/components/ui/button";
  import { Separator } from "$lib/components/ui/separator";
  import { Plus, X } from "lucide-svelte";
  import DetailedStrategy from "./strategy/DetailedStrategy.svelte";
  import { type FilterType, type ValidateStrategy } from "./common";
  import type { components } from "./api";
  import {
    DETAILED_STRATEGY_ODDS_COMPARISON_TYPES,
    detailed_strategy_stringify,
  } from "./strategy/strategy";

  export let filters: FilterType[];
  export let strategy: { value: string; label: string };

  export let preset_mode = false;
  let strategy_alert = false;
  let strategy_error_message = "";
  let filter_types = [
    { value: "TotalUsers", label: "Total users" },
    { value: "DelaySeconds", label: "Delay seconds" },
    { value: "DelayPercentage", label: "Delay percentage" },
  ];
  const PRESET_STRATEGY = { value: "Preset", label: "Preset" };
  const SPECIFIC_STRATEGY = { value: "Specific", label: "Specific" };
  let preset_strategy = "";
  let filters_alert = false;
  let filters_error_message = "";
  let strategy_component_instance: { validate(): ValidateStrategy };
  let strategy_type = {
    value: undefined,
    label: undefined,
    component: undefined,
  };
  let strategy_types = [
    {
      label: "Detailed",
      value: "detailed",
      component: DetailedStrategy,
    },
  ];
  let strategy_props = {};

  function selected_strategy_change(v: any) {
    strategy_type = v;
    strategy_type.component = strategy_types.find(
      (s) => s.value == v.value,
    )?.component;
  }

  export function set_filters_strategy(
    config: components["schemas"]["StreamerConfigRefWrapper"],
  ) {
    if (typeof config._type === "string") {
      strategy = SPECIFIC_STRATEGY;
      strategy_type = strategy_types.find(
        (a) => a.value == Object.keys(config.config.strategy)[0],
      );
      filters = config.config.filters.map((a) => {
        const key = Object.keys(a)[0];
        return {
          value: key,
          label: key,
          quantity: a[key],
        };
      });

      switch (strategy_type.value) {
        case "detailed": {
          strategy_props = {
            detailed_odds: config.config.strategy["detailed"].detailed?.map(
              (x) => ({
                data: detailed_strategy_stringify(x),
                _type: DETAILED_STRATEGY_ODDS_COMPARISON_TYPES.find(
                  (a) => a.value == x._type,
                ),
                error: undefined,
              }),
            ),
            default_odds: detailed_strategy_stringify(
              config.config.strategy["detailed"].default,
            ),
          };
          break;
        }
      }
    } else {
      strategy = PRESET_STRATEGY;
      preset_strategy = config._type.Preset;
    }
  }

  export function get_config():
    | components["schemas"]["ConfigType"]
    | undefined {
    if (strategy.value == "Preset") {
      if (preset_strategy.length == 0) {
        strategy_error_message = "Preset strategy is empty";
        strategy_alert = true;
        return;
      } else {
        strategy_alert = false;
      }

      return {
        Preset: preset_strategy,
      };
    } else {
      if (strategy_type.component == undefined) {
        strategy_error_message = "Specific strategy type is not selected";
        strategy_alert = true;
        return;
      } else {
        strategy_alert = false;
      }

      const { status, data } = strategy_component_instance.validate();
      if (!status) {
        return;
      }

      filters_alert = false;
      for (const v of filters) {
        if (v.quantity === undefined) {
          filters_alert = true;
          filters_error_message = "Value not specified for filter (s)";
          return;
        }

        if (v.value === undefined || v.value === "") {
          filters_alert = true;
          filters_error_message = "Filter type not selected";
          return;
        }
      }

      return {
        Specific: {
          strategy: data,
          // @ts-ignore
          filters: filters.map((a) => ({ [a.value]: parseFloat(a.quantity) })),
        },
      };
    }
  }
</script>

<div>
  <slot />
  <div class="flex flex-col m-4 items-center">
    {#if strategy_alert}
      <ErrorAlert message={strategy_error_message} />
    {/if}
    <div class="flex items-center gap-4 mb-4">
      Strategy
      <Select.Root bind:selected={strategy} disabled={preset_mode}>
        <Select.Trigger class="w-36">
          <Select.Value placeholder="Strategy type" />
        </Select.Trigger>
        <Select.Content>
          <Select.Item value={PRESET_STRATEGY.value}
            >{PRESET_STRATEGY.label}</Select.Item
          >
          <Select.Item value={SPECIFIC_STRATEGY.value}
            >{SPECIFIC_STRATEGY.label}</Select.Item
          >
        </Select.Content>
      </Select.Root>
      {#if strategy.value == "Preset"}
        <Input
          type="text"
          bind:value={preset_strategy}
          placeholder="Strategy name"
          class="w-48"
        />
      {:else}
        <Select.Root
          selected={strategy_type}
          onSelectedChange={(v) => selected_strategy_change(v)}
        >
          <Select.Trigger class="w-36">
            <Select.Value placeholder="Strategy" />
          </Select.Trigger>
          <Select.Content>
            {#each strategy_types as st}
              <Select.Item value={st.value}>{st.label}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      {/if}
    </div>
    {#if strategy.value == "Specific"}
      <svelte:component
        this={strategy_type.component}
        bind:this={strategy_component_instance}
        {...strategy_props}
      />
      <div class="flex m-4 items-center">
        Filters
        <Button
          variant="outline"
          class="rounded-full w-10 h-10 p-0 ml-4"
          on:click={() =>
            (filters = [
              ...filters,
              { value: "", label: "", quantity: undefined },
            ])}
        >
          <Plus class="rounded-full w-10 h-10" size={4} />
        </Button>
      </div>
      <div class="w-full">
        {#if filters_alert}
          <ErrorAlert message={filters_error_message} />
        {/if}
        {#each filters as f, index}
          <div class="flex m-4 gap-1">
            <Select.Root bind:selected={f}>
              <Select.Trigger>
                <Select.Value placeholder="Filter type" />
              </Select.Trigger>
              <Select.Content>
                {#each filter_types as ft}
                  <Select.Item value={ft.value}>{ft.label}</Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
            <Input
              type="number"
              bind:value={f.quantity}
              placeholder="Value"
              class="max-w-1/2"
            />
            <Button
              variant="outline"
              class="rounded-full w-10 h-10 p-0"
              on:click={() => {
                filters.splice(index, 1);
                filters = filters;
              }}
            >
              <X class="rounded-full w-10 h-10" size={4} />
            </Button>
          </div>
          {#if index + 1 != filters.length}
            <Separator />
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</div>
