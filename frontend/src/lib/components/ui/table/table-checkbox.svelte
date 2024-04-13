<script lang="ts">
  import { Checkbox } from "$lib/components/ui/checkbox";
  import type { Writable } from "svelte/store";

  export let selected_row: Writable<string>;
  export let row_id: string;

  let selected_row_id: string;
  let swap = false;
  let from_event = false;

  selected_row.subscribe(s => {
    if (s != row_id && s != "") {
      swap = false;
      from_event = true;
    }
    selected_row_id = s;
  });


  $: {
    if (from_event) {
      from_event = false;
    } else {
      if (swap) {
        selected_row.set(row_id)
      } else {
        selected_row.set('')
      }
    }

  }
</script>

<Checkbox bind:checked={swap} />
