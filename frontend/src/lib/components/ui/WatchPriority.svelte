<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Select from "$lib/components/ui/select";
  import { Button } from "$lib/components/ui/button";
  import SortableList from "$lib/components/ui/SortableList.svelte";
  import { onMount } from "svelte";
  import {
    get_watch_priority,
    set_watch_priority,
    streamers,
    type Streamer,
  } from "../../../common";
  import { Menu } from "lucide-svelte";
  import { Toaster, toast } from "svelte-sonner";
  import type { Selected } from "bits-ui";
  import { get } from "svelte/store";

  export let add_watch_priority = false;
  export let remove_watch_priority = false;
  export let view_edit_watch_priority = false;
  export let dialog = false;

  let watch_priority: string[] = [];
  let streamers_list: Streamer[] = [];
  const sort_priority_list = (ev) => {
    watch_priority = ev.detail;
  };
  let selected_streamer: Selected<string> = { value: "", label: "" };

  onMount(async () => {
    watch_priority = await get_watch_priority();
  });

  $: if (dialog) {
    (async () => {
      if (add_watch_priority) {
        streamers_list = $streamers.filter(
          (x) => watch_priority.find((y) => x.name === y) === undefined,
        );
      } else if (remove_watch_priority) {
        streamers_list = $streamers.filter(
          (x) => watch_priority.find((y) => x.name === y) !== undefined,
        );
      }
    })();
  }

  let action = "";
  let action_title = "";
  $: if (view_edit_watch_priority) {
    action = "Save";
    action_title = "View / Edit watch priority";
  }

  $: if (add_watch_priority) {
    action = "Add";
    action_title = "Add streamer to watch priority";
  }

  $: if (remove_watch_priority) {
    action = "Remove";
    action_title = "Remove streamer from watch priority";
  }

  $: if (!dialog) {
    view_edit_watch_priority = false;
    add_watch_priority = false;
    remove_watch_priority = false;
  }

  async function perform_action() {
    let msg = "Watch priority updated";
    try {
      switch (action) {
        case "Add":
          await set_watch_priority([
            ...watch_priority,
            selected_streamer.value,
          ]);
          break;
        case "Remove":
          await set_watch_priority(
            watch_priority.filter((x) => x !== selected_streamer.value),
          );
          break;
        case "Save":
          await set_watch_priority(watch_priority);
          break;
      }
    } catch (err) {
      msg = `Failed to update watch priority: ${err}`;
    }
    selected_streamer = { value: "", label: "" };
    dialog = false;
    toast(msg);
    watch_priority = await get_watch_priority();
  }
</script>

<div>
  <Dialog.Root bind:open={dialog}>
    <Dialog.Content class="lg:!min-w-[25%] md:!min-w-[100%]">
      <Dialog.Header>
        <Dialog.Title class="mb-4">{action_title}</Dialog.Title>
      </Dialog.Header>
      {#if view_edit_watch_priority}
        <SortableList
          list={watch_priority}
          on:sort={sort_priority_list}
          let:item
        >
          <div class="flex cursor-pointer border-2 rounded-md p-2">
            <p class="mr-1">{item}</p>
            {#if $streamers.find((x) => x.data.info.live && x.data.info.channelName == item) !== undefined}
              <div
                class="self-center ml-1 flex h-2 w-2 items-center justify-center rounded-full bg-green-600"
              ></div>
            {/if}
            <div class="flex-grow"></div>
            <Menu />
          </div>
        </SortableList>
      {:else}
        <Select.Root bind:selected={selected_streamer}>
          <Select.Trigger>
            <Select.Value placeholder="Streamer" />
          </Select.Trigger>
          <Select.Content>
            {#each streamers_list as p}
              <Select.Item value={p.name}>{p.name}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      {/if}
      <div class="text-center mt-2">
        <Button on:click={perform_action}>{action}</Button>
      </div>
    </Dialog.Content>
  </Dialog.Root>
  <Toaster />
</div>
