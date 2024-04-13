<script lang="ts">
  import { CircleAlert } from "lucide-svelte";
  import * as Alert from "./alert";

  export let title = "Error";
  export let message = "";
</script>
<Alert.Root class="border-red-500 mb-4">
    <CircleAlert class="h-4 w-4" />
    <Alert.Title>{title}</Alert.Title>
    <Alert.Description>{message}</Alert.Description>
</Alert.Root>