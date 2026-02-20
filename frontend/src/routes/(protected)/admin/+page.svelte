<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client';
	import { authStore } from '$stores/auth';
	import type { User } from '$types/index';

	interface InviteItem {
		id: string;
		email: string;
		role: 'admin' | 'user';
		used: boolean;
		setup_link: string;
		expires_at: string;
		created_at: string;
	}

	let users: User[] = $state([]);
	let invites: InviteItem[] = $state([]);
	let error = $state('');
	let success = $state('');
	let currentUserId = $state('');

	let inviteEmail = $state('');
	let inviteRole: 'admin' | 'user' = $state('user');
	let inviting = $state(false);
	let copiedId = $state('');

	onMount(() => {
		const unsub = authStore.subscribe((state) => {
			if (state.user?.role !== 'admin') {
				goto('/');
				return;
			}
			currentUserId = state.user.id;
		});

		loadUsers();
		loadInvites();
		return unsub;
	});

	async function loadUsers() {
		try {
			users = await api.get<User[]>('/api/admin/users');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load users';
		}
	}

	async function loadInvites() {
		try {
			invites = await api.get<InviteItem[]>('/api/admin/invites');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load invites';
		}
	}

	async function sendInvite() {
		if (!inviteEmail.trim()) return;
		error = '';
		inviting = true;
		try {
			await api.post('/api/admin/invites', { email: inviteEmail.trim(), role: inviteRole });
			inviteEmail = '';
			success = 'Invite sent!';
			setTimeout(() => (success = ''), 3000);
			await loadInvites();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to send invite';
		} finally {
			inviting = false;
		}
	}

	async function updateRole(userId: string, newRole: 'admin' | 'user') {
		try {
			await api.put(`/api/admin/users/${userId}/role`, { role: newRole });
			await loadUsers();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to update role';
		}
	}

	async function deleteUser(userId: string, username: string) {
		if (!confirm(`Delete user "${username}"? This cannot be undone.`)) return;
		try {
			await api.delete(`/api/admin/users/${userId}`);
			users = users.filter((u) => u.id !== userId);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete user';
		}
	}

	function formatDate(dateStr: string): string {
		return new Date(dateStr).toLocaleDateString();
	}

	function isExpired(expiresAt: string): boolean {
		return new Date() > new Date(expiresAt);
	}

	async function copyLink(id: string, link: string) {
		await navigator.clipboard.writeText(link);
		copiedId = id;
		setTimeout(() => (copiedId = ''), 2000);
	}
</script>

<div class="flex h-full flex-col">
	<div class="border-b border-border px-6 py-4">
		<h1 class="text-lg font-semibold">Admin Panel</h1>
		<p class="text-sm text-muted-foreground">Manage users and send invites</p>
	</div>

	<div class="flex-1 overflow-y-auto p-6">
		<div class="mx-auto max-w-3xl space-y-8">
			{#if error}
				<div class="rounded-lg bg-destructive/10 px-4 py-3 text-sm text-destructive">
					{error}
					<button onclick={() => (error = '')} class="ml-2 underline">Dismiss</button>
				</div>
			{/if}

			{#if success}
				<div class="rounded-lg bg-success/10 px-4 py-3 text-sm text-success">
					{success}
				</div>
			{/if}

			<!-- Invite User -->
			<section class="space-y-4">
				<h2 class="text-base font-semibold">Invite New User</h2>
				<div class="rounded-xl border border-border bg-card p-4 space-y-3">
					<div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
						<div class="space-y-1 sm:col-span-2">
							<label for="inviteEmail" class="text-xs text-muted-foreground">Email</label>
							<input
								id="inviteEmail"
								type="email"
								bind:value={inviteEmail}
								placeholder="user@example.com"
								class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
							/>
						</div>
						<div class="space-y-1">
							<label for="inviteRole" class="text-xs text-muted-foreground">Role</label>
							<select
								id="inviteRole"
								bind:value={inviteRole}
								class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none"
							>
								<option value="user">User</option>
								<option value="admin">Admin</option>
							</select>
						</div>
					</div>
					<button
						onclick={sendInvite}
						disabled={inviting || !inviteEmail.trim()}
						class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
					>
						{inviting ? 'Sending...' : 'Send Invite'}
					</button>
				</div>
			</section>

			<!-- Pending Invites -->
			{#if invites.length > 0}
				<section class="space-y-4">
					<h2 class="text-base font-semibold">Invites</h2>
					<div class="space-y-2">
						{#each invites as invite}
							<div class="rounded-lg border border-border bg-card px-4 py-3 space-y-2">
								<div class="flex items-center justify-between">
									<div>
										<p class="text-sm font-medium">{invite.email}</p>
										<p class="text-xs text-muted-foreground">
											Role: {invite.role} &middot; Sent: {formatDate(invite.created_at)}
										</p>
									</div>
									<div>
										{#if invite.used}
											<span class="rounded-full bg-success/10 px-2 py-0.5 text-xs text-success">
												Accepted
											</span>
										{:else if isExpired(invite.expires_at)}
											<span
												class="rounded-full bg-destructive/10 px-2 py-0.5 text-xs text-destructive"
											>
												Expired
											</span>
										{:else}
											<span class="rounded-full bg-warning/10 px-2 py-0.5 text-xs text-warning">
												Pending
											</span>
										{/if}
									</div>
								</div>
								{#if !invite.used && !isExpired(invite.expires_at)}
									<div class="flex items-center gap-2">
										<input
											type="text"
											readonly
											value={invite.setup_link}
											class="flex-1 rounded-md border border-input bg-background px-2 py-1 text-xs text-muted-foreground outline-none"
										/>
										<button
											onclick={() => copyLink(invite.id, invite.setup_link)}
											class="shrink-0 rounded-md border border-input px-2 py-1 text-xs hover:bg-accent"
										>
											{copiedId === invite.id ? 'Copied!' : 'Copy'}
										</button>
									</div>
								{/if}
							</div>
						{/each}
					</div>
				</section>
			{/if}

			<!-- Users -->
			<section class="space-y-4">
				<h2 class="text-base font-semibold">Users</h2>
				<div class="rounded-xl border border-border">
					<div
						class="grid grid-cols-[1fr_1fr_auto_auto] gap-4 border-b border-border px-4 py-3 text-xs font-medium text-muted-foreground"
					>
						<span>Username</span>
						<span>Email</span>
						<span>Role</span>
						<span>Actions</span>
					</div>

					{#if users.length === 0}
						<p class="px-4 py-8 text-center text-sm text-muted-foreground">No users found.</p>
					{:else}
						{#each users as user}
							<div
								class="grid grid-cols-[1fr_1fr_auto_auto] items-center gap-4 border-b border-border px-4 py-3 last:border-0"
							>
								<div class="min-w-0">
									<p class="truncate text-sm font-medium">{user.username}</p>
								</div>
								<div class="min-w-0">
									<p class="truncate text-sm text-muted-foreground">{user.email}</p>
								</div>
								<div>
									{#if user.id === currentUserId}
										<span
											class="rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary"
										>
											{user.role}
										</span>
									{:else}
										<select
											value={user.role}
											onchange={(e) =>
												updateRole(
													user.id,
													(e.target as HTMLSelectElement).value as 'admin' | 'user'
												)}
											class="rounded-md border border-input bg-background px-2 py-1 text-xs outline-none"
										>
											<option value="user">user</option>
											<option value="admin">admin</option>
										</select>
									{/if}
								</div>
								<div>
									{#if user.id !== currentUserId}
										<button
											onclick={() => deleteUser(user.id, user.username)}
											class="rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
										>
											Delete
										</button>
									{:else}
										<span class="text-xs text-muted-foreground">You</span>
									{/if}
								</div>
							</div>
						{/each}
					{/if}
				</div>
			</section>
		</div>
	</div>
</div>
