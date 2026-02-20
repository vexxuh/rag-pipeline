<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { api } from '$api/client';
	import { authStore } from '$stores/auth';
	import type { User } from '$types/index';

	let username = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let error = $state('');
	let loading = $state(false);

	let token = $derived(new URL($page.url).searchParams.get('token') ?? '');

	async function handleSetup(e: SubmitEvent) {
		e.preventDefault();
		error = '';

		if (!token) {
			error = 'Invalid invite link. No token provided.';
			return;
		}

		if (password !== confirmPassword) {
			error = 'Passwords do not match';
			return;
		}

		if (password.length < 8) {
			error = 'Password must be at least 8 characters';
			return;
		}

		if (username.trim().length < 3) {
			error = 'Username must be at least 3 characters';
			return;
		}

		loading = true;

		try {
			const res = await api.post<{ token: string; user: User }>('/api/auth/setup', {
				token,
				username,
				password
			});
			authStore.login(res.user, res.token);
			goto('/chat');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Setup failed';
		} finally {
			loading = false;
		}
	}
</script>

<div class="flex min-h-screen items-center justify-center p-4">
	<div class="w-full max-w-md space-y-6">
		<div class="text-center">
			<h1 class="text-3xl font-bold">RAG Pipeline</h1>
			<p class="mt-2 text-muted-foreground">Set up your account</p>
		</div>

		{#if !token}
			<div class="rounded-xl border border-border bg-card p-6 text-center">
				<p class="text-destructive">Invalid invite link. Please check the link from your email.</p>
				<a
					href="/login"
					class="mt-4 inline-block text-sm text-primary underline-offset-4 hover:underline"
				>
					Go to login
				</a>
			</div>
		{:else}
			<form
				onsubmit={handleSetup}
				class="space-y-4 rounded-xl border border-border bg-card p-6"
			>
				{#if error}
					<div class="rounded-lg bg-destructive/10 px-4 py-3 text-sm text-destructive">
						{error}
					</div>
				{/if}

				<div class="space-y-2">
					<label for="username" class="text-sm font-medium">Username</label>
					<input
						id="username"
						type="text"
						bind:value={username}
						required
						class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
						placeholder="Choose a username"
					/>
				</div>

				<div class="space-y-2">
					<label for="password" class="text-sm font-medium">Password</label>
					<input
						id="password"
						type="password"
						bind:value={password}
						required
						minlength="8"
						class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
						placeholder="Min 8 characters"
					/>
				</div>

				<div class="space-y-2">
					<label for="confirmPassword" class="text-sm font-medium">Confirm Password</label>
					<input
						id="confirmPassword"
						type="password"
						bind:value={confirmPassword}
						required
						class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
						placeholder="Confirm your password"
					/>
				</div>

				<button
					type="submit"
					disabled={loading}
					class="w-full rounded-lg bg-primary px-4 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
				>
					{loading ? 'Setting up...' : 'Set Up Account'}
				</button>
			</form>
		{/if}
	</div>
</div>
