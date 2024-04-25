<script lang="ts">
  import { ModeWatcher, toggleMode } from "mode-watcher";
  import { Sun, Moon } from "lucide-svelte";
  import * as Tabs from "$lib/components/ui/tabs";
  import { Button } from "$lib/components/ui/button";
  import Points from "./Points.svelte";
  import Setup from "./Setup.svelte";
  import Predictions from "./Predictions.svelte";
  import { onMount } from "svelte";
  import { get_streamers, streamers } from "./common";
  import Logs from "./Logs.svelte";

  onMount(async () => {
    streamers.set(await get_streamers());
  });
</script>

<main class="container min-w-full min-h-full pt-4 font-sans">
  <ModeWatcher />
  <Tabs.Root value="Points">
    <Tabs.List>
      <Tabs.Trigger value="Points">Points</Tabs.Trigger>
      <Tabs.Trigger value="Predictions">Predictions</Tabs.Trigger>
      <Tabs.Trigger value="Setup">Setup</Tabs.Trigger>
      <Tabs.Trigger value="Logs">Logs</Tabs.Trigger>
    </Tabs.List>
    <div class="flex justify-between items-center">
      <div></div>
      <h1 class="text-4xl mb-4 inline">Twitch points miner</h1>
      <Button
        on:click={toggleMode}
        variant="outline"
        size="icon"
        class="float-right"
      >
        <Sun
          class="h-[1.2rem] w-[1.2rem] rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0"
        />
        <Moon
          class="absolute h-[1.2rem] w-[1.2rem] rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100"
        />
        <span class="sr-only">Toggle theme</span>
      </Button>
    </div>
    <Tabs.Content value="Points">
      <Points />
    </Tabs.Content>
    <Tabs.Content value="Predictions">
      <Predictions />
    </Tabs.Content>
    <Tabs.Content value="Setup">
      <Setup />
    </Tabs.Content>
    <Tabs.Content value="Logs">
      <Logs />
    </Tabs.Content>
  </Tabs.Root>
</main>
