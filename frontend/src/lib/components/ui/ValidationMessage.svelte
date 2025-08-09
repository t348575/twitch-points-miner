<script lang="ts">
  import { ValidationMessage } from "@felte/reporter-svelte";
  import ErrorAlert from "./ErrorAlert.svelte";
  import { isIterable } from "$common";
  let for_entry: string;

  export { for_entry as for };
</script>

<ValidationMessage for={for_entry} let:messages>
  {#if isIterable(messages) || (messages && "type" in messages && messages.type)}
    <ErrorAlert content={messages}>
      {#if isIterable(messages)}
        <ul aria-live="polite">
          {#each messages ?? [] as message}
            <li>{message}</li>
          {/each}
        </ul>
      {:else if messages && "type" in messages && messages.type}
        <span>{messages.type}</span>
      {/if}
    </ErrorAlert>
  {/if}
</ValidationMessage>
