<script lang="ts">
  import { Input } from "$ui/input";
  import { Label } from "$ui/label";
  import * as z from "zod";
  import { validator } from "@felte/validator-zod";
  import { createForm } from "felte";
  import { createEventDispatcher } from "svelte";

  let id = window.crypto.randomUUID();
  const schema = z.object({
    [id]: z.number().min(0),
  });
  const { form, data, validate, errors } = createForm<z.z.infer<typeof schema>>(
    {
      onSubmit() {},
      extend: validator({ schema }),
    },
  );

  const events = createEventDispatcher();

  $: if ($errors[id]) {
    events("error", $errors[id]);
  }

  $: if (value) {
    $data[id] = value;
  }


  export let value: number;
  export let name: string;
  export { validate, data, errors };
</script>

<form use:form>
  <Label>{name}</Label>
  <Input
    type="number"
    bind:value={$data[id]}
    name={id}
    placeholder={name}
    class="max-w-1/2"
  />
</form>
