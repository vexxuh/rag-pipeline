<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';

	onMount(() => {
		const token = localStorage.getItem('auth_token');
		if (!token) {
			goto('/login');
		} else {
			goto('/chat');
		}
	});
</script>

<div class="flex h-screen items-center justify-center">
	<div class="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent">
	</div>
</div>
