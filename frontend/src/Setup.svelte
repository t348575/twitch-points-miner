<script lang="ts">
  import * as Table from "$lib/components/ui/table";
  import * as Select from "$lib/components/ui/select";
  import * as Menubar from "$lib/components/ui/menubar";
  import * as Card from "$lib/components/ui/card";
  import * as Dialog from "$lib/components/ui/dialog";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import { Input } from "$lib/components/ui/input";
  import { Button } from "$lib/components/ui/button";
  import ErrorAlert from "$lib/components/ui/ErrorAlert.svelte";
  import { Toaster } from "$lib/components/ui/sonner";
  import { toast } from "svelte-sonner";
  import {
    createRender,
    createTable,
    Render,
    Subscribe,
  } from "svelte-headless-table";
  import { addSortBy, addSelectedRows } from "svelte-headless-table/plugins";
  import { get, writable } from "svelte/store";
  import { onMount } from "svelte";
  import {
    get_streamers,
    mine_streamer,
    remove_streamer,
    type FilterType,
    type Streamer,
    save_streamer_config,
    get_presets,
    add_or_update_preset,
    delete_preset,
    get_watching,
  } from "./common";
  import { ArrowUpDown, SlidersHorizontal, X } from "lucide-svelte";
  import Config from "./Config.svelte";
  import type { components } from "./api";
  import WatchPriority from "./WatchPriority.svelte";
  import { ScrollArea } from "$lib/components/ui/scroll-area";

  let data = writable<Streamer[]>([]);

  let selected_row = writable<string>("");
  let selected_row_id: string = "";
  selected_row.subscribe((s) => (selected_row_id = s));
  let watching: components["schemas"]["StreamerState"][] = [];

  const table = createTable(data, {
    sort: addSortBy(),
    select: addSelectedRows(),
  });
  const columns = table.createColumns([
    table.column({
      accessor: "id",
      header: "Select",
      cell: ({ row }) => {
        return createRender(Table.TableCheckbox, {
          selected_row,
          row_id: row.id,
        });
      },
    }),
    table.column({
      accessor: ({ data }) => data.info.channelName,
      header: "Channel",
    }),
    table.column({
      accessor: ({ data }) => data.points,
      header: "Points",
    }),
    table.column({
      accessor: ({ data }) => data.info.live,
      header: "Live",
    }),
  ]);

  onMount(async () => {
    data.set(await get_streamers());
    watching = (await get_watching()).slice(0, 2);
  });

  const { headerRows, pageRows, tableAttrs, tableBodyAttrs, pluginStates } =
    table.createViewModel(columns);
  const { selectedDataIds } = pluginStates.select;

  let preset_name = "";
  let preset_dialog = false;
  let config_dialog = false;
  let remove_dialog = false;
  let remove_preset_mode = false;
  let channel_name = "";
  let add_streamer_alert = false;
  let error_message = "";
  let filters: FilterType[] = [];
  let strategy = {
    value: "",
    label: "",
  };
  let view_edit = false;
  let config_component: Config;
  const BLANK_PRESET = { value: undefined, label: undefined, data: undefined };
  let preset = BLANK_PRESET;
  let preset_list: {
    value: string;
    label: string;
    data: components["schemas"]["StreamerConfig"];
  }[] = [];

  let add_watch_priority = false;
  let remove_watch_priority = false;
  let view_edit_watch_priority = false;
  let watch_priority = false;

  $: if (!remove_dialog) {
    remove_preset_mode = false;
    preset = BLANK_PRESET;
  }

  $: if (!config_dialog) {
    error_message = "";
    add_streamer_alert = false;
    channel_name = "";
    filters = [];
    strategy = { label: "", value: "" };
  }

  $: if (!preset_dialog) {
    filters = [];
    strategy = { label: "", value: "" };
    preset = BLANK_PRESET;
  }

  $: if (config_dialog && view_edit && config_component != undefined) {
    config_component.set_filters_strategy(
      get(data)[selected_row_id].data.config,
    );
  }

  $: if (preset_dialog && view_edit && config_component != undefined) {
    const item = preset_list.find((a) => a.value == preset.value);
    if (item != undefined) {
      let entry = JSON.parse(JSON.stringify(item));
      config_component.set_filters_strategy({
        _type: "Specific",
        config: {
          prediction: entry.data.prediction
        },
      });
    }
  }

  async function r_streamer() {
    try {
      await remove_streamer(get(data)[selected_row_id].data.info.channelName);
    } catch (err) {
      toast(`Failed to remove streamer: ${err}`);
    }
    data.set(await get_streamers());
  }

  async function add_streamer() {
    let exists = get(data).find((a) => a.data.info.channelName == channel_name);
    if (exists != undefined) {
      error_message = "Streamer already in system";
      add_streamer_alert = true;
      return;
    } else if (channel_name.length == 0) {
      error_message = "Streamer name is empty";
      add_streamer_alert = true;
    } else {
      add_streamer_alert = false;
    }

    let config = config_component.get_config();
    if (config === undefined) {
      return;
    }

    try {
      await mine_streamer(channel_name, config);
    } catch (err) {
      error_message = err as string;
      add_streamer_alert = true;
      return;
    }
    config_dialog = false;
    data.set(await get_streamers());
  }

  async function save_config() {
    let config = config_component.get_config();
    if (config === undefined) {
      return;
    }

    try {
      await save_streamer_config(
        get(data)[selected_row_id].data.info.channelName,
        config,
      );
    } catch (err) {
      toast(`Failed to remove streamer: ${err}`);
      return;
    }
    config_dialog = false;
    data.set(await get_streamers());
  }

  async function view_edit_config() {
    view_edit = true;
    config_dialog = true;
    await load_presets();
  }

  async function load_presets() {
    preset_list = [];
    const res = await get_presets();
    for (const v in res) {
      // @ts-ignore
      preset_list.push({ value: v, label: v, data: res[v] });
    }
    preset_list = preset_list;
  }

  async function view_edit_preset() {
    view_edit = true;
    preset_dialog = true;

    await load_presets();
  }

  function add_preset_button() {
    view_edit = false;
    preset_dialog = true;
    strategy = {
      value: "Specific",
      label: "Specific",
    };
  }

  async function save_preset() {
    let config = config_component.get_config();
    if (config === undefined) {
      return;
    }

    let name;
    if (view_edit) {
      name = preset.value;
    } else {
      name = preset_name;
    }
    try {
      await add_or_update_preset(name, config.Specific);
    } catch (err) {
      toast(`Failed to save preset: ${err}`);
      return;
    }
    preset_dialog = false;
    data.set(await get_streamers());
  }

  async function remove_preset() {
    remove_dialog = true;
    remove_preset_mode = true;

    await load_presets();
  }

  async function remove_preset_button() {
    try {
      await delete_preset(preset.value);
      preset_dialog = false;
    } catch (err) {
      toast(`Failed to remove preset: ${err}`);
      return;
    }
    data.set(await get_streamers());
  }

  async function add_streamer_button() {
    view_edit = false;
    config_dialog = true;
    await load_presets();
  }
</script>

<div class="flex flex-col justify-center items-center">
  <div class="mb-4 sm:w-3/4 md:w-1/2">
    <Menubar.Root class="float-right">
      <Menubar.Menu>
        <Menubar.Trigger>Streamer</Menubar.Trigger>
        <Menubar.Content>
          <Menubar.Item on:click={add_streamer_button}>Add</Menubar.Item>
          <Menubar.Item
            disabled={selected_row_id.length == 0}
            on:click={() => (remove_dialog = true)}>Remove</Menubar.Item
          >
        </Menubar.Content>
      </Menubar.Menu>
      <Menubar.Menu>
        <Menubar.Trigger>Config</Menubar.Trigger>
        <Menubar.Content>
          <Menubar.Item
            disabled={selected_row_id.length == 0}
            on:click={view_edit_config}>View / Edit</Menubar.Item
          >
        </Menubar.Content>
      </Menubar.Menu>
      <Menubar.Menu>
        <Menubar.Trigger>Presets</Menubar.Trigger>
        <Menubar.Content>
          <Menubar.Item on:click={add_preset_button}>Add</Menubar.Item>
          <Menubar.Item on:click={remove_preset}>Remove</Menubar.Item>
          <Menubar.Item on:click={view_edit_preset}>View / Edit</Menubar.Item>
        </Menubar.Content>
      </Menubar.Menu>
      <Menubar.Menu>
        <Menubar.Trigger>Watch priority</Menubar.Trigger>
        <Menubar.Content>
          <Menubar.Item
            on:click={() => {
              add_watch_priority = true;
              watch_priority = true;
            }}>Add</Menubar.Item
          >
          <Menubar.Item
            on:click={() => {
              remove_watch_priority = true;
              watch_priority = true;
            }}>Remove</Menubar.Item
          >
          <Menubar.Item
            on:click={() => {
              view_edit_watch_priority = true;
              watch_priority = true;
            }}>View / Edit</Menubar.Item
          >
        </Menubar.Content>
      </Menubar.Menu>
    </Menubar.Root>
  </div>
  <div class="flex w-full justify-center">
    <div class="mr-2">
      <Card.Root>
        <Card.Header>
          <Card.Title>Currently watching</Card.Title>
        </Card.Header>
        <Card.Content>
          {#each watching as w}
            <a href={`https://twitch.tv/${w.info.channelName}`} target="_blank">{w.info.channelName}</a>
            <br />
          {/each}
        </Card.Content>
      </Card.Root>
    </div>
    <div class="rounded-md border sm:w-3/4 md:w-1/2">
      <Table.Root {...$tableAttrs}>
        <Table.Header>
          {#each $headerRows as headerRow}
            <Subscribe rowAttrs={headerRow.attrs()}>
              <Table.Row>
                {#each headerRow.cells as cell (cell.id)}
                  <Subscribe
                    attrs={cell.attrs()}
                    let:attrs
                    props={cell.props()}
                    let:props
                  >
                    <Table.Head {...attrs} class="[&:has([role=checkbox])]:pl-3">
                      {#if cell.id === "Points"}
                        <Button variant="ghost" on:click={props.sort.toggle}>
                          <Render of={cell.render()} />
                          <ArrowUpDown class={"ml-2 h-4 w-4"} />
                        </Button>
                      {:else}
                        <Render of={cell.render()} />
                      {/if}
                    </Table.Head>
                  </Subscribe>
                {/each}
              </Table.Row>
            </Subscribe>
          {/each}
        </Table.Header>
        <Table.Body {...$tableBodyAttrs}>
          {#each $pageRows as row (row.id)}
            <Subscribe rowAttrs={row.attrs()} let:rowAttrs>
              <Table.Row
                {...rowAttrs}
                data-state={$selectedDataIds[row.id] && "selected"}
              >
                {#each row.cells as cell (cell.id)}
                  <Subscribe attrs={cell.attrs()} let:attrs>
                    <Table.Cell {...attrs}>
                      {#if cell.id == "Config"}
                        <Button variant="ghost">
                          <SlidersHorizontal />
                        </Button>
                      {:else}
                        <Render of={cell.render()} />
                      {/if}
                    </Table.Cell>
                  </Subscribe>
                {/each}
              </Table.Row>
            </Subscribe>
          {/each}
        </Table.Body>
      </Table.Root>
    </div>
  </div>

  <Dialog.Root bind:open={config_dialog}>
    <Dialog.Content class="lg:!min-w-[50%] md:!min-w-[100%] mt-2 !max-h-[98%]">
      <Dialog.Header>
        <Dialog.Title class="mb-4">
          {#if view_edit}
            Streamer config: {get(data)[selected_row_id].data.info.channelName}
          {:else}
            Add streamer
          {/if}
        </Dialog.Title>
      </Dialog.Header>
      <div class="flex flex-col items-center">
        {#if !view_edit}
          {#if add_streamer_alert}
            <ErrorAlert message={error_message} />
          {/if}
          <Input
            type="text"
            bind:value={channel_name}
            placeholder="Channel name"
            class="max-w-xs"
          />
        {/if}
        <Config
          bind:filters
          bind:strategy
          bind:this={config_component}
          {preset_list}
        />
        <Button
          class="mt-4 max-w-24"
          on:click={view_edit ? save_config : add_streamer}
        >
          {#if view_edit}
            Save config
          {:else}
            Add streamer
          {/if}
        </Button>
      </div>
    </Dialog.Content>
  </Dialog.Root>

  <Dialog.Root bind:open={preset_dialog}>
    <Dialog.Content class="lg:!min-w-[50%] md:!min-w-[100%]">
        <Dialog.Header>
          <Dialog.Title>Preset config</Dialog.Title>
        </Dialog.Header>
        <div class ="flex flex-col items-center !max-h-[90vh] !min-h-[10vh]">
          <ScrollArea class="!max-h-[90vh] !min-h-[10vh] w-full">
            <div class="flex flex-col items-center">
              {#if view_edit}
                <Select.Root bind:selected={preset}>
                  <Select.Trigger class="my-2 max-w-xs">
                    <Select.Value placeholder="Preset" />
                  </Select.Trigger>
                  <Select.Content>
                    {#each preset_list as p}
                      <Select.Item value={p.value}>{p.label}</Select.Item>
                    {/each}
                  </Select.Content>
                </Select.Root>
                {#if preset.value != undefined}
                  <Config
                    bind:filters
                    bind:strategy
                    bind:this={config_component}
                    preset_mode={true}
                  />
                {/if}
              {:else}
                {#if add_streamer_alert}
                  <ErrorAlert message={error_message} />
                {/if}
                <Input
                  type="text"
                  bind:value={preset_name}
                  placeholder="Preset name"
                />
                <Config
                  bind:filters
                  bind:strategy
                  bind:this={config_component}
                  preset_mode={true}
                />
              {/if}
            </div>
          </ScrollArea>
          <Button class="mt-4 max-w-24" on:click={save_preset}>
            Save preset
          </Button>
        </div>
    </Dialog.Content>
  </Dialog.Root>

  <AlertDialog.Root bind:open={remove_dialog}>
    <AlertDialog.Content>
      <AlertDialog.Header>
        <AlertDialog.Title>
          Remove {#if remove_preset_mode}
            preset
          {:else}
            streamer
          {/if}
        </AlertDialog.Title>
        <AlertDialog.Description>
          {#if remove_preset_mode}
            <Select.Root bind:selected={preset}>
              <Select.Trigger class="my-2">
                <Select.Value placeholder="Preset" />
              </Select.Trigger>
              <Select.Content>
                {#each preset_list as p}
                  <Select.Item value={p.value}>{p.label}</Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
            {#if preset.value != undefined}
              <p class="mx-1">
                Are you sure you want to remove {preset.value}?
              </p>
            {/if}
          {:else}
            Are you sure you want to remove {get(data)[selected_row_id].data
              .info.channelName}?
            <br />
            <p class="font-bold inline">Note:</p>
            Analytics will not be deleted.
          {/if}
        </AlertDialog.Description>
      </AlertDialog.Header>
      <AlertDialog.Footer>
        <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
        <AlertDialog.Action
          on:click={remove_preset_mode ? remove_preset_button : r_streamer}
          disabled={remove_preset_mode && preset.value == undefined}
          >Remove</AlertDialog.Action
        >
      </AlertDialog.Footer>
    </AlertDialog.Content>
  </AlertDialog.Root>
  <WatchPriority
    bind:view_edit_watch_priority
    bind:add_watch_priority
    bind:remove_watch_priority
    bind:dialog={watch_priority}
  />
  <Toaster />
</div>
