<script lang="ts">
  import ErrorAlert from "$lib/components/ui/ErrorAlert.svelte";
  import { getObjectByPath, isIterable, transformErrors, type NestedObject } from "$common";
  import { derived, type Readable } from "svelte/store";

  export let errors: Readable<any>;
  export let path: string;
  let errorArray = [];

  let firstTime = true;
  const unchanged = derived(errors, ($errors, set) => {
    let timeout: ReturnType<typeof setTimeout>;
    let lastValue = getObjectByPath($errors as NestedObject, path);
    if (lastValue === undefined) {
      firstTime = false;
    }

    const checkUnchanged = (value: any) => {
      clearTimeout(timeout);
      timeout = setTimeout(() => {
          if (lastValue === value) {
              set(true);
          } else {
              set(false);
          }
      }, 50);
      lastValue = value;
      set(firstTime);
  };

    checkUnchanged($errors);
    return () => {
        clearTimeout(timeout);
    };
  });
  $: errorArray = transformErrors(getObjectByPath($errors, path));
</script>

{#if isIterable(errorArray) && errorArray.length > 0 && $unchanged}
  <ErrorAlert content={errorArray}>
    <ul aria-live="polite">
      {#each errorArray ?? [] as message}
        <li>{message.key}: {message.value}</li>
      {/each}
    </ul>
  </ErrorAlert>
{/if}
