<script lang="ts">
  import * as Card from "$lib/components/ui/card";
  import { onMount } from "svelte";
  import { get_logs } from "./common";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import { Button } from "$lib/components/ui/button";
  import { ChevronLeft, ChevronRight } from "lucide-svelte";
  import { Input } from "$lib/components/ui/input";

  let text = "";
  let page = 0;
  let page_size = 30;
  onMount(async () => {
    await render_logs();
  });

  async function render_logs() {
    text = await get_logs(page, page_size);
  }
</script>

<div class="flex flex-col">
  <Card.Root>
    <Card.Content class="max-h-[80vh]">
      <ScrollArea orientation="both">
        <pre class="max-h-[80vh]">
  {@html text}
        </pre>
      </ScrollArea>
    </Card.Content>
  </Card.Root>

  <div class="flex gap-1 self-center content-center mt-1">
    <Input
      type="number"
      min="1"
      placeholder="Lines per page"
      bind:value={page_size}
    />
    <Button
      variant="outline"
      on:click={() => {
        page++;
        render_logs();
      }}><ChevronLeft /></Button
    >
    <p class="p-2">{page + 1}</p>
    <Button
      variant="outline"
      on:click={() => {
        page--;
        render_logs();
      }}
      disabled={page === 0}><ChevronRight /></Button
    >
  </div>
</div>
