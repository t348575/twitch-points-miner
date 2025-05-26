<script lang="ts">
  import { Button } from "$ui/button";
  import { ChevronDown, ChevronUp } from "lucide-svelte";
  export let name: string;

  let open = false;
</script>

<div class="flex flex-col">
  <div
    class="flex items-center self-center lg:w-64 sm:w-96 justify-between space-x-4 px-4"
  >
    <h4 class="text-sm font-semibold">{name}</h4>
    <Button
      variant="ghost"
      size="sm"
      class="w-9 p-0"
      on:click={() => (open = !open)}
    >
      {#if open}
        <ChevronUp class="h-4 w-4" />
      {:else}
        <ChevronDown class="h-4 w-4" />
      {/if}
      <span class="sr-only">Toggle</span>
    </Button>
  </div>
  <div class={(open ? "show block" : "hide hidden")}>
    <slot />
  </div>
</div>

<style>
  @keyframes slideaway {
    from {
      transform: translateY(0);
    }
    to {
      transform: translateY(-20%);
    }
  }
  @keyframes slideinto {
    from {
      transform: translateY(-20%);
    }
    to {
      transform: translateY(0);
    }
  }
  .hide {
    animation: slideaway 70ms ease-in-out;
  }
  .show {
    animation: slideinto 70ms ease-in-out;
  }
</style>
