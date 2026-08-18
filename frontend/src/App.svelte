<script lang="ts">
  import { ModeWatcher, toggleMode } from "mode-watcher";
  import { Sun, Moon } from "lucide-svelte";
  import { Button } from "$lib/components/ui/button";
  import Points from "./routes/Points.svelte";
  import Setup from "./routes/Setup.svelte";
  import Predictions from "./routes/Predictions.svelte";
  import { onMount } from "svelte";
  import { get_streamers, streamers } from "./common";
  import Logs from "./routes/Logs.svelte";
  import Router, { location } from "svelte-spa-router";

  onMount(async () => {
    streamers.set(await get_streamers());
  });

  const routes = {
    '/': Points,
    '/predictions': Predictions,
    '/setup': Setup,
    '/logs': Logs
  }

  const tab_class = "data-[state=active]:bg-background data-[state=active]:text-foreground rounded-sm px-3 py-1.5 text-sm shadow-sm inline-flex items-center justify-center h-8";
</script>

<main class="container min-w-full min-h-full pt-4 font-sans">
  <ModeWatcher />
  <div class="flex flex-col-reverse md:flex-row justify-center items-center gap-4 relative md:mb-4">
    <div class="flex justify-center md:justify-start w-full md:w-auto md:flex-1">
      <div class="bg-muted rounded-md p-1 h-10 inline-flex items-center text-muted-foreground overflow-x-auto max-w-full">
        <Button variant="ghost" class={tab_class} data-state={$location === '/' ? 'active' : 'inactive'} href="#/">Points</Button>
        <Button variant="ghost" class={tab_class} data-state={$location === '/predictions' ? 'active' : 'inactive'} href="#/predictions">Predictions</Button>
        <Button variant="ghost" class={tab_class} data-state={$location === '/setup' ? 'active' : 'inactive'} href="#/setup">Setup</Button>
        <Button variant="ghost" class={tab_class} data-state={$location === '/logs' ? 'active' : 'inactive'} href="#/logs">Logs</Button>
      </div>
    </div>
    <h1 class="text-3xl md:text-4xl mb-0 w-full md:w-auto text-center font-extrabold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-indigo-600 to-purple-600 dark:from-indigo-400 dark:to-purple-400">
      Twitch Points Miner
    </h1>
    <div class="flex justify-end w-full md:w-auto md:flex-1 absolute top-0 right-0 md:static p-2 md:p-0">
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
  </div>
  <Router {routes}/>
</main>
