<script lang="ts">
  import { Input } from "$ui/input";
  import * as Select from "$ui/select";
  import { Button } from "$ui/button";
  import { Separator } from "$ui/separator";
  import { X, Plus } from "lucide-svelte";
  import { asSelected } from "$common";
  import {
    DETAILED_STRATEGY_ODDS_COMPARISON_TYPES as oddsTypes,
    dsSchema,
  } from "./";
  import * as z from "zod";
  import { createForm } from "felte";
  import { Label } from "$ui/label";
  import type { Selected } from "bits-ui";
  import ValidationMessage from "$ui/ValidationMessage.svelte";
  import GroupedValidationMessage from "$ui/GroupedValidationMessage.svelte";
  import { validator } from "@felte/validator-zod";
  import SimpleCollapsible from "$ui/SimpleCollapsible.svelte";
  import type { components } from "src/api";

  const { form, data, setFields, setData, validate, errors } = createForm<z.z.infer<typeof dsSchema>>({
    onSubmit() {},
    extend: validator({ schema: dsSchema }),
    initialValues: {
      default: {
        max_percentage: undefined,
        min_percentage: undefined,
        points: {
          max_value: undefined,
          percent: undefined,
        },
      },
      detailed: [],
    },
  });

  export let detailed: components["schemas"]["Detailed"] | undefined =
    undefined;
  $: if (detailed) {
    setFields('detailed', detailed.detailed);
    setFields('default', detailed.default);
    $data; // why does it not work without this?
  }

  function addDetailed() {
    // @ts-ignore
    // prettier-ignore
    setFields('detailed', [...$data.detailed, { type: "", attempt_rate: undefined, threshold: undefined, points: { max_value: undefined, percent: undefined } }], true)
  }

  function selectDetailed(
    v: Selected<keyof typeof oddsTypes> | undefined,
    index: number,
  ) {
    setFields(`detailed.${index}.type`, v?.value, true);
  }

  function removeDetailed(index: number) {
    $data.detailed.splice(index, 1);
    setFields("detailed", $data.detailed, true);
  }

  $: console.log(detailed, $data)

  export { validate, data };
</script>

<form class="flex flex-col" use:form>
  <SimpleCollapsible name="Default odds">
    <GroupedValidationMessage
      errors={errors}
      path="default"
    />
    <div class="grid grid-rows-2 grid-cols-12 gap-1 items-center">
      <div class="col-span-6 mt-4">
        <Label for="default.max_percentage">Max percentage</Label>
        <Input
          type="number"
          placeholder="Max percentage"
          id="default.max_percentage"
          name="default.max_percentage"
          bind:value={$data.default.max_percentage}
        />
      </div>
      <div class="col-span-6 mt-4">
        <Label for="default.min_percentage">Min percentage</Label>
        <Input
          type="number"
          placeholder="Min percentage"
          id="default.min_percentage"
          name="default.min_percentage"
          bind:value={$data.default.min_percentage}
        />
      </div>
      <div class="col-span-6">
        <Label for="default.points.max_value"
          >Max points value</Label
        >
        <Input
          type="number"
          placeholder="Max value"
          id="default.points.max_value"
          name="default.points.max_value"
          bind:value={$data.default.points.max_value}
        />
      </div>
      <div class="col-span-6">
        <Label for="default.points.percent"
          >Points percentage</Label
        >
        <Input
          type="number"
          placeholder="Percentage"
          id="default.points.percent"
          name="default.points.percent"
          bind:value={$data.default.points.percent}
        />
      </div>
    </div>
  </SimpleCollapsible>

  <SimpleCollapsible name="Detailed odds">
    <div class="flex m-4 items-center justify-center">
      Detailed odds
      <Button
        variant="outline"
        class="rounded-full w-10 h-10 p-0 ml-4"
        on:click={addDetailed}
      >
        <Plus class="rounded-full w-10 h-10" size={4} />
      </Button>
    </div>
    {#each $data.detailed as f, index}
      <GroupedValidationMessage
        errors={errors}
        path="detailed.{index}"
      />
      <div class="grid grid-rows-3 grid-cols-11 gap-1 items-center my-4 mx-1">
        <div class="col-span-5">
          <Label for="detailed.{index}.threshold"
            >Odds threshold</Label
          >
          <Input
            type="number"
            placeholder="Threshold"
            id="detailed.{index}.threshold"
            name="detailed.{index}.threshold"
            bind:value={f.threshold}
          />
        </div>
        <div class="col-span-5">
          <ValidationMessage for="detailed.{index}.type" />
          <Label for="detailed.{index}.type">Threshold type</Label
          >
          <Select.Root
            selected={asSelected(f.type, oddsTypes[f.type])}
            onSelectedChange={(v) => selectDetailed(v, index)}
            name="detailed.{index}._type"
          >
            <Select.Trigger class="col-span-5">
              <Select.Value placeholder="Type" />
            </Select.Trigger>
            <Select.Content>
              {#each Object.entries(oddsTypes) as d}
                <Select.Item value={d[0]}>{d[1]}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
        <Button
          variant="outline"
          class="rounded-full w-10 h-10 p-0 row-span-3 col-span-1 place-self-center ml-1"
          on:click={() => removeDetailed(index)}
        >
          <X class="rounded-full w-10 h-10" size={4} />
        </Button>
        <div class="col-span-10">
          <Label for="detailed.{index}.attempt_rate"
            >Attempt rate</Label
          >
          <Input
            type="number"
            placeholder="Attempt rate"
            name="detailed.{index}.attempt_rate"
            bind:value={f.attempt_rate}
          />
        </div>
        <div class="col-span-5">
          <Label for="detailed.{index}.points.max_value"
            >Max points value</Label
          >
          <Input
            type="number"
            placeholder="Max value"
            name="detailed.{index}.points.max_value"
            bind:value={f.points.max_value}
          />
        </div>
        <div class="col-span-5">
          <Label for="detailed.{index}.points.percent"
            >Points percentage</Label
          >
          <Input
            type="number"
            placeholder="Percentage"
            name="detailed.{index}.points.percent"
            bind:value={f.points.percent}
          />
        </div>
      </div>
      {#if index + 1 != $data.detailed.length}
        <Separator />
      {/if}
    {/each}
  </SimpleCollapsible>
</form>
