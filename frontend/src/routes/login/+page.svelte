<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$api/client';
	import { authStore } from '$stores/auth';
	import type { User } from '$types/index';

	let email = $state('');
	let password = $state('');
	let error = $state('');
	let loading = $state(false);

	async function handleLogin(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		loading = true;

		try {
			const res = await api.post<{ token: string; user: User }>('/api/auth/login', {
				email,
				password
			});
			authStore.login(res.user, res.token);
			goto('/chat');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Login failed';
		} finally {
			loading = false;
		}
	}
</script>

<div class="flex min-h-screen items-center justify-center p-4">
	<div class="w-full max-w-md space-y-6">
		<div class="text-center">
			<h1 class="text-3xl font-bold">RAG Pipeline</h1>
			<p class="mt-2 text-muted-foreground">Sign in to your account</p>
		</div>

		<form onsubmit={handleLogin} class="space-y-4 rounded-xl border border-border bg-card p-6">
			{#if error}
				<div class="rounded-lg bg-destructive/10 px-4 py-3 text-sm text-destructive">
					{error}
				</div>
			{/if}

			<div class="space-y-2">
				<label for="email" class="text-sm font-medium">Email</label>
				<input
					id="email"
					type="email"
					bind:value={email}
					required
					class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
					placeholder="you@example.com"
				/>
			</div>

			<div class="space-y-2">
				<label for="password" class="text-sm font-medium">Password</label>
				<input
					id="password"
					type="password"
					bind:value={password}
					required
					class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
					placeholder="Enter your password"
				/>
			</div>

			<button
				type="submit"
				disabled={loading}
				class="w-full rounded-lg bg-primary px-4 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
			>
				{loading ? 'Signing in...' : 'Sign in'}
			</button>

			<p class="text-center text-sm text-muted-foreground">
				Need an account? Ask your admin for an invite.
			</p>
		</form>
	</div>
</div>
