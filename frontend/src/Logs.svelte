<script lang="ts">
  import * as Card from "$lib/components/ui/card";
  import { onMount } from "svelte";
  import { get_logs } from "./common";

  let text = "";
  onMount(async () => {
    text = await get_logs();
  });
</script>

<Card.Root>
  <Card.Content>
    <pre>
{@html text}
    </pre>
  </Card.Content>
</Card.Root>
