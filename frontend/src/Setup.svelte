<script lang="ts">
  import * as Table from "$lib/components/ui/table";
  import * as Menubar from "$lib/components/ui/menubar";
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
  } from "./common";
  import { ArrowUpDown, SlidersHorizontal, X } from "lucide-svelte";
  import Config from "./Config.svelte";

  let data = writable<Streamer[]>([]);

  let selected_row = writable<string>("");
  let selected_row_id: string = "";
  selected_row.subscribe((s) => (selected_row_id = s));

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
  });

  const { headerRows, pageRows, tableAttrs, tableBodyAttrs, pluginStates } =
    table.createViewModel(columns);
  const { selectedDataIds } = pluginStates.select;

  let config_dialog = false;
  let remove_streamer_dialog = false;
  let channel_name = "";
  let add_streamer_alert = false;
  let error_message = "";
  let filters: FilterType[] = [];
  let strategy = {
    value: "",
    label: "",
    component: undefined,
  };
  let view_edit = false;
  let config_component: Config;

  $: if (!config_dialog) {
    error_message = "";
    add_streamer_alert = false;
    channel_name = "";
    filters = [];
    strategy = { label: "", value: "", component: undefined };
  }

  $: if (config_dialog && view_edit && config_component != undefined) {
    config_component.set_filters_strategy(
      get(data)[selected_row_id].data.config
    );
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
        config
      );
    } catch (err) {
      toast(`Failed to remove streamer: ${err}`);
      return;
    }
    config_dialog = false;
    data.set(await get_streamers());
  }

  function view_edit_config() {
    view_edit = true;
    config_dialog = true;
  }
</script>

<div class="flex flex-col justify-center items-center">
  <div class="mb-4 sm:w-3/4 md:w-1/2">
    <Menubar.Root class="float-right">
      <Menubar.Menu>
        <Menubar.Trigger>Streamer</Menubar.Trigger>
        <Menubar.Content>
          <Menubar.Item
            on:click={() => {
              view_edit = false;
              config_dialog = true;
            }}>Add</Menubar.Item
          >
          <Menubar.Item
            disabled={selected_row_id.length == 0}
            on:click={() => (remove_streamer_dialog = true)}
            >Remove</Menubar.Item
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
    </Menubar.Root>
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

  <Dialog.Root bind:open={config_dialog}>
    <Dialog.Content>
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
          />
        {/if}
        <Config bind:filters bind:strategy bind:this={config_component} />
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

  <AlertDialog.Root bind:open={remove_streamer_dialog}>
    <AlertDialog.Content>
      <AlertDialog.Header>
        <AlertDialog.Title>Remove streamer</AlertDialog.Title>
        <AlertDialog.Description>
          Are you sure you want to remove {get(data)[selected_row_id].data.info
            .channelName}?
          <br />
          <p class="font-bold inline">Note:</p>
           Analytics will not be deleted.
        </AlertDialog.Description>
      </AlertDialog.Header>
      <AlertDialog.Footer>
        <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
        <AlertDialog.Action on:click={r_streamer}>Remove</AlertDialog.Action>
      </AlertDialog.Footer>
    </AlertDialog.Content>
  </AlertDialog.Root>
  <Toaster />
</div>
