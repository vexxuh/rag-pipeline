<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	onMount(() => {
		goto('/admin?tab=settings', { replaceState: true });
	});
</script>

<div class="flex h-full items-center justify-center">
	<p class="text-sm text-muted-foreground">Redirecting to admin settings...</p>
</div>
