<script lang="ts">
  import * as Select from "$ui/select";
  import { Button } from "$ui/button";
  import { Separator } from "$ui/separator";
  import { Plus, X } from "lucide-svelte";
  import {
    asSelected,
    transformErrors,
    typedObjectKeys,
    type PresetList,
  } from "$common";
  import type { components } from "src/api";
  import { DetailedStrategy, External } from "./strategy";
  import type { Selected } from "bits-ui";
  import { Switch } from "$ui/switch";
  import { Label } from "$ui/label";
  import { createForm } from "felte";
  import * as z from "zod";
  import { validator } from "@felte/validator-zod";
  import { reporter } from "@felte/reporter-svelte";
  import ValidationMessage from "$ui/ValidationMessage.svelte";
  import SimpleCollapsible from "$ui/SimpleCollapsible.svelte";
  import { onMount, tick } from "svelte";
  import { toast } from "svelte-sonner";
  import { get, type Readable } from "svelte/store";
  import ErrorAlert from "../ErrorAlert.svelte";
  import CommonFilter from "./CommonFilter.svelte";

  export let strategy: { value: "Preset" | "Specific"; label: string };

  export let preset_list: PresetList = {};

  type ExternalComponent = { validate: () => any; data: Readable<any> };
  let strategy_component_instance: ExternalComponent;
  interface StrategyType {
    label: string;
    value: string;
    component: ExternalComponent;
  }
  const strategy_types = {
    Detailed: {
      label: "Detailed",
      component: DetailedStrategy,
    },
    External: {
      label: "External",
      component: External,
    },
  };
  let strategy_type: StrategyType;
  const filter_types = {
    TotalUsers: { label: "Total users" },
    DelaySeconds: { label: "Delay seconds" },
    DelayPercentage: { label: "Delay percentage" },
    External: { label: "External (JS)" },
  };

  const [firstFilterType, ...otherFilterTypes] = typedObjectKeys(filter_types);
  const [firstPresetType, ...otherPresetTypes] = typedObjectKeys(preset_list);
  const [firstStrategyType, ...otherStrategyTypes] =
    typedObjectKeys(strategy_types);

  const schema = z.object({
    follow_raid: z.boolean(),
    strategy: z.discriminatedUnion("base", [
      z.object({
        base: z.literal("Specific"),
        type: z.enum([firstStrategyType!, ...otherStrategyTypes]),
        filters: z
          .object({
            type: z.enum([firstFilterType!, ...otherFilterTypes]),
            data: z.any(),
            props: z.any(),
            // instance: z.any(),
            error: z.any(),
          })
          .array(),
      }),
      z.object({
        base: z.literal("Preset"),
        // @ts-ignore
        preset: z.enum([firstPresetType!, ...otherPresetTypes]),
      }),
    ]),
  });

  const { form, data, setFields, errors, validate } = createForm<
    z.z.infer<typeof schema>
  >({
    extend: [validator({ schema }), reporter],
    initialValues: {
      follow_raid: true,
      strategy: {},
    },
  });

  onMount(() => {
    $data.strategy.base = strategy?.value;
  });

  export let preset_mode = false;

  const baseStrategies = {
    Preset: "Preset",
    Specific: "Specific",
  };
  let strategy_props = {};

  function addFilter() {
    if ($data.strategy.base === "Specific") {
      // @ts-ignore
      // prettier-ignore
      $data.strategy.filters = [...$data.strategy.filters, { type: undefined, props: undefined, error: undefined, data: undefined }];
    }
  }

  function selectedFilter(
    type: keyof typeof filter_types,
  ): Selected<string> | undefined {
    if (!type) return undefined;
    return {
      value: type,
      label: type.length === 0 ? "" : filter_types[type].label,
    };
  }

  function selectFilter(v: Selected<string> | undefined, index: number) {
    if ($data.strategy.base === "Specific" && v) {
      // @ts-ignore
      $data.strategy.filters[index].type = v.value;
    }
  }

  async function removeFilter(index: number) {
    if ($data.strategy.base === "Specific") {
      $data.strategy.filters.splice(index, 1);
      $data.strategy.filters = $data.strategy.filters;
    }
  }

  export async function set_filters_strategy(
    config: components["schemas"]["StreamerConfigRefWrapper"],
  ) {

    if ($data.strategy.base === "Specific") {
      const len = $data.strategy.filters.length;
      for (let i = 0; i < len; i++) {
        removeFilter(0);
      }
    }

    if (typeof config.type === "string") {
      setFields("follow_raid", config.config.follow_raid);
      setFields("strategy.base", "Specific");
      // @ts-ignore
      // prettier-ignore
      strategy_type = strategy_types[Object.keys(config.config.prediction.strategy)[0]];

      // @ts-ignore
      setFields(
        "strategy.filters",
        config.config.prediction.filters.map((a) => {
          const key = Object.keys(a)[0];
          return {
            type: key,
            instance: undefined,
            error: undefined,
            data: undefined,
          };
        }),
      );

      setFields(
        "strategy.type",
        Object.keys(config.config.prediction.strategy)[0],
      );
      switch (Object.keys(config.config.prediction.strategy)[0]) {
        case "Detailed": {
          strategy_props = {
            // @ts-ignore
            detailed: config.config.prediction.strategy.Detailed,
          };
          break;
        }
        case "External": {
          strategy_props = {
            // @ts-ignore
            external: config.config.prediction.strategy.External,
          };
          break;
        }
      }

      await tick();
      const len = $data.strategy.filters.length;
      for (let i = 0; i < len; i++) {
        const key = $data.strategy.filters[i].type;
        switch (key) {
          case "External": {
            // @ts-ignore
            $data.strategy.filters[i].props = {
              external: config.config.prediction.filters[i].External,
            };
            break;
          }
          default: {
            // @ts-ignore
            $data.strategy.filters[i].props = {
              value: config.config.prediction.filters[i][key],
            };
            break;
          }
        }
      }
    } else {
      setFields("strategy.base", "Preset");
      setFields("strategy.preset", config.type.Preset);
    }
  }

  export async function get_config(): Promise<
    components["schemas"]["ConfigType"] | undefined
  > {
    const strategyErrors = transformErrors(
      await strategy_component_instance.validate(),
    );

    if (strategyErrors.length > 0) {
      toast("Errors are there in the form!");
      console.log(strategyErrors);
      return;
    }

    const configErrors = validate();
    if (transformErrors(configErrors).length > 0) {
      toast("Errors are there in the form!");
      console.log(transformErrors(configErrors));
      return;
    }

    if (strategy.value == "Preset") {
      if ($data.strategy.base == "Specific") {
        return;
      }

      console.log($data);

      return {
        Preset: {},
      };
    } else {
      if ($data.strategy.base == "Preset") {
        return;
      }

      console.log($data);
      return {
        Specific: {
          follow_raid: $data.follow_raid,
          prediction: {
            filters: $data.strategy.filters.map((a, index) => {
              switch (a.type) {
                case "External": {
                  return { External: get(filters[index].data) };
                }
                default: {
                  return {
                    [a.type]: Object.values(get(filters[index].data))[0],
                  };
                }
              }
            }),
            strategy: (() => {
              switch ($data.strategy.type) {
                case "External": {
                  return {
                    External: get(strategy_component_instance.data),
                  };
                }
                case "Detailed": {
                  return {
                    Detailed: get(strategy_component_instance.data),
                  };
                }
              }
            })(),
          },
        },
      };
      // const { status, data } = strategy_component_instance.validate();
      // if (!status) {
      //   return;
      // }

      // filters_alert = false;
      // for (const v of filters) {
      //   if (v.data === undefined) {
      //     filters_alert = true;
      //     filters_error_message = "Value not specified for filter (s)";
      //     return;
      //   }

      //   if (v.value === undefined || v.value === "") {
      //     filters_alert = true;
      //     filters_error_message = "Filter type not selected";
      //     return;
      //   }
      // }

      // return {
      //   Specific: {
      //     follow_raid,
      //     prediction: {
      //       strategy: data,
      //       // @ts-ignore
      //       filters: filters.map((a) => ({
      //         [a.value]: parseFloat(a.quantity),
      //       })),
      //     },
      //   },
      // };
    }
  }

  function selectStrategy(
    v: Selected<keyof typeof baseStrategies> | undefined,
  ) {
    setFields("strategy.base", v!.value, true);
  }

  function selectPreset(v: Selected<keyof typeof preset_list> | undefined) {
    setFields("strategy.preset", v!.value, true);
  }

  function selectStrategyType(
    v: Selected<keyof typeof strategy_types> | undefined,
  ) {
    setFields("strategy.type", v!.value, true);
    if ($data.strategy.base == "Specific") {
      if (v!.value in strategy_types) {
        // @ts-ignore
        strategy_type = {
          value: v!.value,
          ...strategy_types[v!.value],
        };
      }

      if (!$data.strategy.filters) {
        $data.strategy.filters = [];
      }
    }
  }
</script>

<div>
  <slot />
  <form class="flex flex-col m-4 max-w-full" use:form>
    <div class="flex self-center gap-4 mb-4">
      <Label for="follow_raid" class="self-center">Follow raid</Label>
      <Switch
        id="follow_raid"
        name="follow_raid"
        bind:checked={$data.follow_raid}
      />
    </div>
    <ValidationMessage for="strategy" />
    <div class="flex items-center gap-4 mb-4">
      Strategy
      <Select.Root
        selected={asSelected(
          $data.strategy.base,
          baseStrategies[$data.strategy.base],
        )}
        onSelectedChange={(v) => selectStrategy(v)}
        name="strategy.base"
        disabled={preset_mode}
      >
        <Select.Trigger class="w-52">
          <Select.Value placeholder="Strategy type" />
        </Select.Trigger>
        <Select.Content class="w-52">
          {#each Object.keys(baseStrategies) as s}
            <Select.Item value={s}>{s}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
      {#if $data.strategy.base == "Preset"}
        <ValidationMessage for="strategy.preset" />
        <Select.Root
          selected={asSelected(
            $data.strategy.preset,
            preset_list[$data.strategy.preset]?.label,
          )}
          onSelectedChange={(v) => selectPreset(v)}
          name="strategy.preset"
        >
          <Select.Trigger class="my-2 max-w-xs">
            <Select.Value placeholder="Preset" />
          </Select.Trigger>
          <Select.Content>
            {#each Object.entries(preset_list) as p}
              <Select.Item value={p[0]}>{p[1].label}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      {:else if $data.strategy.base == "Specific"}
        <ValidationMessage for="strategy.type" />
        <Select.Root
          name="strategy.type"
          selected={{
            value: $data.strategy.type,
            label: strategy_types[$data.strategy.type]?.label,
          }}
          onSelectedChange={(v) => selectStrategyType(v)}
        >
          <Select.Trigger class="w-36">
            <Select.Value placeholder="Strategy" />
          </Select.Trigger>
          <Select.Content>
            {#each Object.entries(strategy_types) as st}
              <Select.Item value={st[0]}>{st[1].label}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      {/if}
    </div>
  </form>
  {#if $data.strategy.base == "Specific" && $data.strategy.type}
    <svelte:component
      this={strategy_type.component}
      bind:this={strategy_component_instance}
      {...strategy_props}
    />
    <SimpleCollapsible name="Filters">
      <div class="flex m-4 place-content-center">
        <span class="self-center">Filters</span>
        <Button
          variant="outline"
          class="rounded-full w-10 h-10 p-0 ml-4"
          on:click={addFilter}
        >
          <Plus class="rounded-full w-10 h-10" size={4} />
        </Button>
      </div>
      <div class="w-full">
        {#each $data.strategy?.filters as f, index}
          <ErrorAlert content={f.error}>
            {f.error}
          </ErrorAlert>
          <div class="flex gap-1 justify-center m-2">
            <div>
              <Label>Filter type</Label>
              <Select.Root
                selected={selectedFilter(f.type)}
                name="strategy.filters.{index}.type"
                onSelectedChange={(v) => selectFilter(v, index)}
              >
                <Select.Trigger class="w-48">
                  <Select.Value placeholder="Filter type" />
                </Select.Trigger>
                <Select.Content>
                  {#each Object.entries(filter_types) as ft}
                    <Select.Item value={ft[0]}>{ft[1].label}</Select.Item>
                  {/each}
                </Select.Content>
              </Select.Root>
            </div>
            {#if f.type !== undefined && f.type !== "External"}
              <CommonFilter
                bind:data={f.data}
                on:error={(e) => (f.error = e.detail)}
                name={filter_types[f.type].label}
                {...f.props}
              />
            {/if}
            <Button
              variant="outline"
              class="rounded-full w-10 h-10 p-0 ml-1 mt-6"
              on:click={() => removeFilter(index)}
            >
              <X class="rounded-full w-10 h-10" size={4} />
            </Button>
          </div>
          {#if f.type === "External"}
            <External bind:data={f.data} {...f.props} />
          {/if}
          {#if index + 1 != $data.strategy.filters.length}
            <Separator class="mt-4" />
          {/if}
        {/each}
      </div>
    </SimpleCollapsible>
  {/if}
</div>
