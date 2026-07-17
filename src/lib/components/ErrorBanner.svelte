<script lang="ts">
  let { message }: { message: string } = $props();
</script>

<p class="error" role="alert">{message}</p>
