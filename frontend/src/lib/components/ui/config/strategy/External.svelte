<svelte:options accessors={true} />
<script lang="ts">
  import * as Select from "$ui/select";
  import { Input } from "$ui/input";
  import type { components } from "src/api";
  import CodeMirror from "svelte-codemirror-editor";
  import { javascript } from "@codemirror/lang-javascript";
  import { githubLight, githubDark } from "@uiw/codemirror-theme-github";
  import { mode } from "mode-watcher";
  import { asSelected, get_external_file } from "$common";
  import { toast } from "svelte-sonner";
  import { Label } from "$ui/label";
  import { externalSchema } from "./";
  import { createForm } from "felte";
  import { validator } from "@felte/validator-zod";
  import * as z from "zod";
  import type { Selected } from "bits-ui";
  import GroupedValidationMessage from "$ui/GroupedValidationMessage.svelte";

  const { form, data, setFields, setData, validate, errors } = createForm<z.z.infer<typeof externalSchema>>({
    onSubmit() {},
    extend: validator({ schema: externalSchema }),
    initialValues: {
      type: undefined,
      data: undefined,
      file_data: undefined,
    },
  });

  const types = ["Inline", "File"];
  export let external: components["schemas"]["External"] | undefined =
    undefined;
  let is_set = false;

  $: if (external) {
    if (external?.type === "File" && !is_set) {
      is_set = true;
      get_external_file(external?.data)
      .then((res) => {
          // @ts-ignore
          external.file_data = res;
          setData(external);
          $data;
        })
        .catch((err) => toast(`Failed to get file data: ${err}`));
    } else {
      setData(external);
      $data;
    }
  }

  function selectType(
    v: Selected<components["schemas"]["ExternalType"]> | undefined,
  ) {
    // @ts-ignore
    setFields("type", v?.value, true);
  }

  export { validate, data };

  // $: console.log($data)
</script>

<form use:form>
  <GroupedValidationMessage errors={errors} path="" />
  <div class="flex justify-center mb-2">
    <div>
      <Label>External type</Label>
      <Select.Root
        selected={asSelected($data.type, $data.type)}
        onSelectedChange={(v) => selectType(v)}
        name="type"
      >
        <Select.Trigger class="w-48">
          <Select.Value placeholder="Filter type" />
        </Select.Trigger>
        <Select.Content>
          {#each types as t}
            <Select.Item value={t}>{t}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
    </div>
  </div>

  {#if $data.type === "Inline"}
    <div class="mt-2">
      <Label>Inline code</Label>
      <CodeMirror
        bind:value={$data.data}
        lang={javascript({ typescript: true })}
        theme={$mode === "dark" ? githubDark : githubLight}
      />
    </div>
  {:else if $data.type === "File"}
    <div>
      <Label for="data.data">File path</Label>
      <Input
        type="text"
        bind:value={$data.data}
        placeholder="File path"
        class="max-w-1/2 mb-2"
        name="data"
        id="data.data"
      />
      <Label class="mt-2">{$data.data}</Label>
      <CodeMirror
        bind:value={$data.file_data}
        lang={javascript({ typescript: true })}
        theme={$mode === "dark" ? githubDark : githubLight}
      />
    </div>
  {/if}
</form>
