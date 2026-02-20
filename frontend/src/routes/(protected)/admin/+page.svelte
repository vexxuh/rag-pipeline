<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { api } from '$api/client';
	import { authStore } from '$stores/auth';
	import type {
		User,
		ConversationLog,
		LogsResponse,
		LogDetail,
		ApiKey,
		LlmPreferences,
		AdminProvider,
		AdminModel,
		EmbedKey,
		CreateEmbedKeyResponse
	} from '$types/index';

	type Role = 'admin' | 'maintainer' | 'user';
	type Tab = 'users' | 'invites' | 'logs' | 'settings' | 'embed';

	interface InviteItem {
		id: string;
		email: string;
		role: Role;
		used: boolean;
		setup_link: string;
		expires_at: string;
		created_at: string;
	}

	let activeTab: Tab = $state('users');
	let users: User[] = $state([]);
	let invites: InviteItem[] = $state([]);
	let error = $state('');
	let success = $state('');
	let currentUserId = $state('');

	// Invite form
	let inviteEmail = $state('');
	let inviteRole: Role = $state('user');
	let inviting = $state(false);
	let copiedId = $state('');

	// Logs state
	let logs: ConversationLog[] = $state([]);
	let logsTotal = $state(0);
	let logsPage = $state(1);
	let logsPerPage = 25;
	let logsUserFilter = $state('');
	let selectedLog: LogDetail | null = $state(null);
	let loadingLogs = $state(false);

	// Settings state
	let providers: AdminProvider[] = $state([]);
	let completionModels: AdminModel[] = $state([]);
	let embeddingModels: AdminModel[] = $state([]);
	let loadingModels = $state(false);
	let apiKeys: ApiKey[] = $state([]);
	let preferences: LlmPreferences = $state({
		preferred_provider: '',
		preferred_model: '',
		preferred_embedding_model: '',
		system_prompt: ''
	});
	let newKeyProvider = $state('');
	let newKeyValue = $state('');
	let saving = $state(false);
	let settingsLoaded = $state(false);

	// Embed state
	let embedKeys: EmbedKey[] = $state([]);
	let embedLoaded = $state(false);
	let showEmbedForm = $state(false);
	let editingEmbedId: string | null = $state(null);
	let rawKeyDisplay: string | null = $state(null);
	let embedSaving = $state(false);
	let embedForm = $state({
		name: '',
		allowed_domains: '',
		system_prompt: '',
		rate_limit: 20,
		widget_title: 'Chat with us',
		primary_color: '#2563eb',
		greeting_message: 'Hello! How can I help you?',
		provider: '',
		model: '',
		api_key: ''
	});
	let copiedSnippetId = $state('');
	let copiedRawKey = $state(false);

	onMount(() => {
		const unsub = authStore.subscribe((state) => {
			if (state.user?.role !== 'admin') {
				goto('/');
				return;
			}
			currentUserId = state.user.id;
		});

		// Handle ?tab= query param for direct navigation
		const tabParam = $page.url.searchParams.get('tab');
		if (tabParam && ['users', 'invites', 'logs', 'settings', 'embed'].includes(tabParam)) {
			switchTab(tabParam as Tab);
		}

		loadUsers();
		loadInvites();
		return unsub;
	});

	// ---- Users ----
	async function loadUsers() {
		try {
			users = await api.get<User[]>('/api/admin/users');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load users';
		}
	}

	async function updateRole(userId: string, newRole: Role) {
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

	// ---- Invites ----
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

	// ---- Logs ----
	async function loadLogs() {
		loadingLogs = true;
		try {
			const params = new URLSearchParams();
			params.set('page', logsPage.toString());
			params.set('per_page', logsPerPage.toString());
			if (logsUserFilter) params.set('user_id', logsUserFilter);

			const resp = await api.get<LogsResponse>(`/api/admin/logs?${params}`);
			logs = resp.conversations;
			logsTotal = resp.total;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load logs';
		} finally {
			loadingLogs = false;
		}
	}

	async function viewConversation(id: string) {
		try {
			selectedLog = await api.get<LogDetail>(`/api/admin/logs/${id}`);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load conversation';
		}
	}

	// ---- Settings ----
	async function loadSettings() {
		if (settingsLoaded) return;
		try {
			const [provs, keys, prefs] = await Promise.all([
				api.get<AdminProvider[]>('/api/settings/providers'),
				api.get<ApiKey[]>('/api/settings/api-keys'),
				api.get<LlmPreferences | null>('/api/settings/preferences')
			]);
			providers = provs;
			apiKeys = keys;
			if (prefs) preferences = prefs;
			settingsLoaded = true;

			if (preferences.preferred_provider) {
				await loadModelsForProvider(preferences.preferred_provider);
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load settings';
		}
	}

	async function loadModelsForProvider(providerId: string) {
		loadingModels = true;
		try {
			const models = await api.get<AdminModel[]>(
				`/api/settings/providers/${providerId}/models`
			);
			completionModels = models.filter((m) => m.model_type === 'completion');
			embeddingModels = models.filter((m) => m.model_type === 'embedding');
		} catch {
			completionModels = [];
			embeddingModels = [];
		} finally {
			loadingModels = false;
		}
	}

	async function onProviderChange() {
		if (preferences.preferred_provider) {
			await loadModelsForProvider(preferences.preferred_provider);
			const defaultCompletion = completionModels.find((m) => m.is_default);
			const defaultEmbedding = embeddingModels.find((m) => m.is_default);
			preferences = {
				...preferences,
				preferred_model: defaultCompletion?.model_id ?? completionModels[0]?.model_id ?? '',
				preferred_embedding_model:
					defaultEmbedding?.model_id ?? embeddingModels[0]?.model_id ?? ''
			};
		} else {
			completionModels = [];
			embeddingModels = [];
			preferences = {
				...preferences,
				preferred_model: '',
				preferred_embedding_model: ''
			};
		}
	}

	async function saveApiKey() {
		if (!newKeyProvider || !newKeyValue.trim()) return;
		error = '';
		try {
			await api.put(`/api/settings/api-keys/${newKeyProvider}`, {
				api_key: newKeyValue.trim()
			});
			newKeyValue = '';
			success = `API key saved for ${getProviderName(newKeyProvider)}`;
			setTimeout(() => (success = ''), 3000);
			apiKeys = await api.get<ApiKey[]>('/api/settings/api-keys');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save API key';
		}
	}

	async function deleteApiKey(provider: string) {
		try {
			await api.delete(`/api/settings/api-keys/${provider}`);
			apiKeys = apiKeys.filter((k) => k.provider !== provider);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete API key';
		}
	}

	async function savePreferences() {
		error = '';
		saving = true;
		try {
			await api.put('/api/settings/preferences', preferences);
			success = 'Preferences saved';
			setTimeout(() => (success = ''), 3000);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save preferences';
		} finally {
			saving = false;
		}
	}

	function getProviderName(id: string): string {
		return providers.find((p) => p.provider_id === id)?.display_name ?? id;
	}

	function hasApiKey(providerId: string): boolean {
		return apiKeys.some((k) => k.provider === providerId);
	}

	// ---- Embed Keys ----
	async function loadEmbedKeys() {
		if (embedLoaded) return;
		try {
			embedKeys = await api.get<EmbedKey[]>('/api/admin/embed-keys');
			embedLoaded = true;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load embed keys';
		}
	}

	function resetEmbedForm() {
		embedForm = {
			name: '',
			allowed_domains: '',
			system_prompt: '',
			rate_limit: 20,
			widget_title: 'Chat with us',
			primary_color: '#2563eb',
			greeting_message: 'Hello! How can I help you?',
			provider: '',
			model: '',
			api_key: ''
		};
		editingEmbedId = null;
		showEmbedForm = false;
		rawKeyDisplay = null;
	}

	function startEditEmbed(key: EmbedKey) {
		editingEmbedId = key.id;
		embedForm = {
			name: key.name,
			allowed_domains: key.allowed_domains.join(', '),
			system_prompt: key.system_prompt,
			rate_limit: key.rate_limit,
			widget_title: key.widget_title,
			primary_color: key.primary_color,
			greeting_message: key.greeting_message,
			provider: key.provider,
			model: key.model,
			api_key: ''
		};
		showEmbedForm = true;
		rawKeyDisplay = null;
	}

	async function saveEmbedKey() {
		if (!embedForm.name.trim()) return;
		embedSaving = true;
		error = '';

		const domains = embedForm.allowed_domains
			.split(',')
			.map((d) => d.trim())
			.filter(Boolean);

		try {
			if (editingEmbedId) {
				await api.put(`/api/admin/embed-keys/${editingEmbedId}`, {
					name: embedForm.name.trim(),
					allowed_domains: domains,
					system_prompt: embedForm.system_prompt,
					rate_limit: embedForm.rate_limit,
					widget_title: embedForm.widget_title,
					primary_color: embedForm.primary_color,
					greeting_message: embedForm.greeting_message,
					provider: embedForm.provider,
					model: embedForm.model,
					api_key: embedForm.api_key || undefined
				});
				success = 'Embed key updated';
			} else {
				const resp = await api.post<CreateEmbedKeyResponse>('/api/admin/embed-keys', {
					name: embedForm.name.trim(),
					allowed_domains: domains,
					system_prompt: embedForm.system_prompt,
					rate_limit: embedForm.rate_limit,
					widget_title: embedForm.widget_title,
					primary_color: embedForm.primary_color,
					greeting_message: embedForm.greeting_message,
					provider: embedForm.provider,
					model: embedForm.model,
					api_key: embedForm.api_key
				});
				rawKeyDisplay = resp.raw_key;
				success = 'Embed key created! Copy the key below - it won\'t be shown again.';
			}

			setTimeout(() => (success = ''), 5000);
			embedLoaded = false;
			await loadEmbedKeys();

			if (!rawKeyDisplay) {
				resetEmbedForm();
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save embed key';
		} finally {
			embedSaving = false;
		}
	}

	async function deleteEmbedKey(id: string, name: string) {
		if (!confirm(`Delete embed key "${name}"? This will disable any widgets using it.`)) return;
		try {
			await api.delete(`/api/admin/embed-keys/${id}`);
			embedKeys = embedKeys.filter((k) => k.id !== id);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete embed key';
		}
	}

	async function toggleEmbedKey(id: string) {
		try {
			const updated = await api.put<EmbedKey>(`/api/admin/embed-keys/${id}/toggle`, {});
			embedKeys = embedKeys.map((k) => (k.id === id ? updated : k));
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to toggle embed key';
		}
	}

	function getEmbedSnippet(key: EmbedKey): string {
		return `<script src="${window.location.origin}/static/widget.js" data-key="PASTE_YOUR_KEY_HERE" data-server="${window.location.origin}"><\/script>`;
	}

	async function copyToClipboard(text: string, id?: string) {
		await navigator.clipboard.writeText(text);
		if (id) {
			copiedSnippetId = id;
			setTimeout(() => (copiedSnippetId = ''), 2000);
		}
	}

	// ---- Helpers ----
	function formatDate(dateStr: string): string {
		return new Date(dateStr).toLocaleDateString();
	}

	function formatDateTime(dateStr: string): string {
		return new Date(dateStr).toLocaleString();
	}

	function isExpired(expiresAt: string): boolean {
		return new Date() > new Date(expiresAt);
	}

	async function copyLink(id: string, link: string) {
		await navigator.clipboard.writeText(link);
		copiedId = id;
		setTimeout(() => (copiedId = ''), 2000);
	}

	function switchTab(tab: Tab) {
		activeTab = tab;
		if (tab === 'logs' && logs.length === 0) {
			loadLogs();
		}
		if (tab === 'settings') {
			loadSettings();
		}
		if (tab === 'embed') {
			loadEmbedKeys();
		}
	}

	let totalPages = $derived(Math.ceil(logsTotal / logsPerPage));
</script>

<div class="flex h-full flex-col">
	<div class="border-b border-border px-6 py-4">
		<h1 class="text-lg font-semibold">Admin Panel</h1>
		<p class="text-sm text-muted-foreground">Manage users, settings, embeds, and view logs</p>
	</div>

	<!-- Tabs -->
	<div class="border-b border-border px-6">
		<div class="flex gap-6">
			{#each [['users', 'Users'], ['invites', 'Invites'], ['logs', 'Logs'], ['settings', 'Settings'], ['embed', 'Embed']] as [tab, label]}
				<button
					onclick={() => switchTab(tab as Tab)}
					class="relative py-3 text-sm font-medium transition-colors {activeTab === tab
						? 'text-foreground'
						: 'text-muted-foreground hover:text-foreground'}"
				>
					{label}
					{#if activeTab === tab}
						<span class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary"></span>
					{/if}
				</button>
			{/each}
		</div>
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

			<!-- Users Tab -->
			{#if activeTab === 'users'}
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
														(e.target as HTMLSelectElement).value as Role
													)}
												class="rounded-md border border-input bg-background px-2 py-1 text-xs outline-none"
											>
												<option value="user">user</option>
												<option value="maintainer">maintainer</option>
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

			<!-- Invites Tab -->
			{:else if activeTab === 'invites'}
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
									<option value="maintainer">Maintainer</option>
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

				{#if invites.length > 0}
					<section class="space-y-4">
						<h2 class="text-base font-semibold">Pending Invites</h2>
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

			<!-- Logs Tab -->
			{:else if activeTab === 'logs'}
				{#if selectedLog}
					<section class="space-y-4">
						<div class="flex items-center gap-3">
							<button
								onclick={() => (selectedLog = null)}
								class="rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent"
							>
								Back
							</button>
							<div>
								<h2 class="text-base font-semibold">{selectedLog.title}</h2>
								<p class="text-xs text-muted-foreground">
									{selectedLog.messages.length} messages &middot;
									{formatDateTime(selectedLog.created_at)}
								</p>
							</div>
						</div>

						<div class="space-y-3">
							{#each selectedLog.messages as msg}
								<div
									class="rounded-lg border border-border p-3 {msg.role === 'user'
										? 'bg-accent/30'
										: 'bg-card'}"
								>
									<div class="mb-1 flex items-center gap-2">
										<span
											class="rounded-full px-2 py-0.5 text-xs font-medium {msg.role === 'user'
												? 'bg-primary/10 text-primary'
												: 'bg-secondary text-secondary-foreground'}"
										>
											{msg.role === 'user' ? 'User' : 'Assistant'}
										</span>
										<span class="text-xs text-muted-foreground">
											{formatDateTime(msg.created_at)}
										</span>
									</div>
									<p class="whitespace-pre-wrap text-sm">{msg.content}</p>
								</div>
							{/each}

							{#if selectedLog.messages.length === 0}
								<p class="py-8 text-center text-sm text-muted-foreground">
									No messages in this conversation.
								</p>
							{/if}
						</div>
					</section>
				{:else}
					<section class="space-y-4">
						<div class="flex items-center justify-between">
							<h2 class="text-base font-semibold">Chat Logs</h2>
							<button
								onclick={loadLogs}
								class="rounded-md border border-input px-3 py-1.5 text-xs hover:bg-accent"
							>
								Refresh
							</button>
						</div>

						<div class="flex gap-3">
							<div class="space-y-1">
								<label for="logUserFilter" class="text-xs text-muted-foreground">Filter by user</label>
								<select
									id="logUserFilter"
									bind:value={logsUserFilter}
									onchange={() => {
										logsPage = 1;
										loadLogs();
									}}
									class="rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none"
								>
									<option value="">All users</option>
									{#each users as user}
										<option value={user.id}>{user.username} ({user.email})</option>
									{/each}
								</select>
							</div>
						</div>

						{#if loadingLogs}
							<div class="flex justify-center py-8">
								<div
									class="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent"
								></div>
							</div>
						{:else if logs.length === 0}
							<p class="py-8 text-center text-sm text-muted-foreground">
								No conversations found.
							</p>
						{:else}
							<div class="rounded-xl border border-border">
								<div
									class="grid grid-cols-[1fr_1fr_auto_auto] gap-4 border-b border-border px-4 py-3 text-xs font-medium text-muted-foreground"
								>
									<span>User</span>
									<span>Title</span>
									<span>Messages</span>
									<span>Updated</span>
								</div>

								{#each logs as log}
									<button
										onclick={() => viewConversation(log.id)}
										class="grid w-full grid-cols-[1fr_1fr_auto_auto] items-center gap-4 border-b border-border px-4 py-3 text-left transition-colors hover:bg-accent/50 last:border-0"
									>
										<div class="min-w-0">
											<p class="truncate text-sm font-medium">{log.username}</p>
											<p class="truncate text-xs text-muted-foreground">{log.email}</p>
										</div>
										<div class="min-w-0">
											<p class="truncate text-sm">{log.title}</p>
										</div>
										<div>
											<span
												class="rounded-full bg-secondary px-2 py-0.5 text-xs font-medium"
											>
												{log.message_count}
											</span>
										</div>
										<div>
											<span class="text-xs text-muted-foreground">
												{formatDate(log.updated_at)}
											</span>
										</div>
									</button>
								{/each}
							</div>

							{#if totalPages > 1}
								<div class="flex items-center justify-between pt-2">
									<span class="text-xs text-muted-foreground">
										{logsTotal} conversations &middot; Page {logsPage} of {totalPages}
									</span>
									<div class="flex gap-2">
										<button
											onclick={() => {
												logsPage = Math.max(1, logsPage - 1);
												loadLogs();
											}}
											disabled={logsPage <= 1}
											class="rounded-md border border-input px-3 py-1 text-xs hover:bg-accent disabled:opacity-50"
										>
											Prev
										</button>
										<button
											onclick={() => {
												logsPage = Math.min(totalPages, logsPage + 1);
												loadLogs();
											}}
											disabled={logsPage >= totalPages}
											class="rounded-md border border-input px-3 py-1 text-xs hover:bg-accent disabled:opacity-50"
										>
											Next
										</button>
									</div>
								</div>
							{/if}
						{/if}
					</section>
				{/if}

			<!-- Settings Tab -->
			{:else if activeTab === 'settings'}
				<!-- API Keys -->
				<section class="space-y-4">
					<div>
						<h2 class="text-base font-semibold">API Keys</h2>
						<p class="text-xs text-muted-foreground">
							Add your API key for the provider you want to use.
						</p>
					</div>

					<div class="rounded-xl border border-border bg-card p-4 space-y-3">
						<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
							<div class="space-y-1">
								<label for="provider" class="text-xs text-muted-foreground">Provider</label>
								<select
									id="provider"
									bind:value={newKeyProvider}
									class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none"
								>
									<option value="">Select provider</option>
									{#each providers as p}
										<option value={p.provider_id}>{p.display_name}</option>
									{/each}
								</select>
							</div>
							<div class="space-y-1">
								<label for="apikey" class="text-xs text-muted-foreground">API Key</label>
								<input
									id="apikey"
									type="password"
									bind:value={newKeyValue}
									placeholder="sk-..."
									class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
								/>
							</div>
						</div>
						<button
							onclick={saveApiKey}
							disabled={!newKeyProvider || !newKeyValue.trim()}
							class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
						>
							Save Key
						</button>
					</div>

					{#if apiKeys.length > 0}
						<div class="space-y-2">
							{#each apiKeys as key}
								<div
									class="flex items-center justify-between rounded-lg border border-border bg-card px-4 py-3"
								>
									<div>
										<p class="text-sm font-medium">{getProviderName(key.provider)}</p>
										<p class="text-xs text-muted-foreground">
											Added {new Date(key.created_at).toLocaleDateString()}
										</p>
									</div>
									<button
										onclick={() => deleteApiKey(key.provider)}
										class="rounded-md px-3 py-1.5 text-xs text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
									>
										Remove
									</button>
								</div>
							{/each}
						</div>
					{/if}
				</section>

				<!-- LLM Preferences -->
				<section class="space-y-4">
					<div>
						<h2 class="text-base font-semibold">LLM Preferences</h2>
						<p class="text-xs text-muted-foreground">
							Choose your provider and models. Models are loaded based on the selected provider.
						</p>
					</div>

					<div class="rounded-xl border border-border bg-card p-4 space-y-5">
						<div class="space-y-1.5">
							<label for="prefProvider" class="text-sm font-medium">Provider</label>
							<select
								id="prefProvider"
								bind:value={preferences.preferred_provider}
								onchange={onProviderChange}
								class="w-full rounded-lg border border-input bg-background px-3 py-2.5 text-sm outline-none ring-ring focus:ring-2"
							>
								<option value="">Select provider</option>
								{#each providers as p}
									<option value={p.provider_id}>
										{p.display_name}{hasApiKey(p.provider_id) ? '' : ' (no key)'}
									</option>
								{/each}
							</select>
							{#if preferences.preferred_provider && !hasApiKey(preferences.preferred_provider)}
								<p class="text-xs text-warning">
									You haven't added an API key for this provider yet. Add one above to start chatting.
								</p>
							{/if}
						</div>

						<div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
							<div class="space-y-1.5">
								<label for="prefModel" class="text-sm font-medium">Completion Model</label>
								{#if loadingModels}
									<div
										class="flex h-10 items-center rounded-lg border border-input bg-background px-3"
									>
										<span class="text-xs text-muted-foreground">Loading models...</span>
									</div>
								{:else if completionModels.length > 0}
									<select
										id="prefModel"
										bind:value={preferences.preferred_model}
										class="w-full rounded-lg border border-input bg-background px-3 py-2.5 text-sm outline-none ring-ring focus:ring-2"
									>
										{#each completionModels as m}
											<option value={m.model_id}>
												{m.display_name}{m.is_default ? ' (default)' : ''}
											</option>
										{/each}
									</select>
								{:else}
									<div
										class="flex h-10 items-center rounded-lg border border-dashed border-input bg-background px-3"
									>
										<span class="text-xs text-muted-foreground">
											{preferences.preferred_provider
												? 'No completion models available'
												: 'Select a provider first'}
										</span>
									</div>
								{/if}
							</div>

							<div class="space-y-1.5">
								<label for="prefEmbedding" class="text-sm font-medium">Embedding Model</label>
								{#if loadingModels}
									<div
										class="flex h-10 items-center rounded-lg border border-input bg-background px-3"
									>
										<span class="text-xs text-muted-foreground">Loading models...</span>
									</div>
								{:else if embeddingModels.length > 0}
									<select
										id="prefEmbedding"
										bind:value={preferences.preferred_embedding_model}
										class="w-full rounded-lg border border-input bg-background px-3 py-2.5 text-sm outline-none ring-ring focus:ring-2"
									>
										<option value="">None</option>
										{#each embeddingModels as m}
											<option value={m.model_id}>
												{m.display_name}{m.is_default ? ' (default)' : ''}
											</option>
										{/each}
									</select>
								{:else}
									<div
										class="flex h-10 items-center rounded-lg border border-dashed border-input bg-background px-3"
									>
										<span class="text-xs text-muted-foreground">
											{preferences.preferred_provider
												? 'No embedding models available'
												: 'Select a provider first'}
										</span>
									</div>
								{/if}
							</div>
						</div>

						<div class="space-y-1.5">
							<label for="systemPrompt" class="text-sm font-medium">System Prompt</label>
							<textarea
								id="systemPrompt"
								bind:value={preferences.system_prompt}
								rows="4"
								placeholder="Customize how the AI behaves when answering your questions..."
								class="w-full rounded-lg border border-input bg-background px-3 py-2.5 text-sm outline-none ring-ring focus:ring-2"
							></textarea>
							<p class="text-xs text-muted-foreground">
								This prompt tells the RAG pipeline its goals and how to respond.
							</p>
						</div>

						<div class="flex items-center justify-between border-t border-border pt-4">
							<p class="text-xs text-muted-foreground">
								{#if preferences.preferred_provider && preferences.preferred_model}
									Using <strong>{getProviderName(preferences.preferred_provider)}</strong> with <strong
										>{preferences.preferred_model}</strong
									>
								{:else}
									No provider/model selected yet.
								{/if}
							</p>
							<button
								onclick={savePreferences}
								disabled={saving || !preferences.preferred_provider || !preferences.preferred_model}
								class="rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
							>
								{saving ? 'Saving...' : 'Save Preferences'}
							</button>
						</div>
					</div>
				</section>

			<!-- Embed Tab -->
			{:else if activeTab === 'embed'}
				{#if rawKeyDisplay}
					<!-- Raw key display (shown once after creation) -->
					<section class="space-y-4">
						<div class="rounded-xl border-2 border-warning bg-warning/5 p-4 space-y-3">
							<h2 class="text-base font-semibold">Save Your Embed Key</h2>
							<p class="text-sm text-muted-foreground">
								This key will only be shown once. Copy it now and store it securely.
							</p>
							<div class="flex items-center gap-2">
								<code
									class="flex-1 rounded-lg border border-input bg-background px-3 py-2 text-sm font-mono break-all"
								>
									{rawKeyDisplay}
								</code>
								<button
									onclick={() => {
										copyToClipboard(rawKeyDisplay ?? '');
										copiedRawKey = true;
										setTimeout(() => (copiedRawKey = false), 2000);
									}}
									class="shrink-0 rounded-lg border border-input px-3 py-2 text-sm hover:bg-accent"
								>
									{copiedRawKey ? 'Copied!' : 'Copy'}
								</button>
							</div>
							<button
								onclick={resetEmbedForm}
								class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
							>
								Done
							</button>
						</div>
					</section>
				{:else if showEmbedForm}
					<!-- Create / Edit form -->
					<section class="space-y-4">
						<div class="flex items-center gap-3">
							<button
								onclick={resetEmbedForm}
								class="rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent"
							>
								Back
							</button>
							<h2 class="text-base font-semibold">
								{editingEmbedId ? 'Edit Embed Key' : 'Create Embed Key'}
							</h2>
						</div>

						<div class="rounded-xl border border-border bg-card p-4 space-y-4">
							<div class="space-y-1.5">
								<label for="embedName" class="text-sm font-medium">Name</label>
								<input
									id="embedName"
									type="text"
									bind:value={embedForm.name}
									placeholder="e.g. Marketing site widget"
									class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
								/>
							</div>

							<div class="space-y-1.5">
								<label for="embedDomains" class="text-sm font-medium">Allowed Domains</label>
								<input
									id="embedDomains"
									type="text"
									bind:value={embedForm.allowed_domains}
									placeholder="example.com, app.example.com (empty = allow all)"
									class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
								/>
								<p class="text-xs text-muted-foreground">
									Comma-separated list of domains. Leave empty to allow from any domain.
								</p>
							</div>

							<div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
								<div class="space-y-1.5">
									<label for="embedTitle" class="text-sm font-medium">Widget Title</label>
									<input
										id="embedTitle"
										type="text"
										bind:value={embedForm.widget_title}
										class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
									/>
								</div>
								<div class="space-y-1.5">
									<label for="embedColor" class="text-sm font-medium">Primary Color</label>
									<div class="flex gap-2">
										<input
											id="embedColor"
											type="color"
											bind:value={embedForm.primary_color}
											class="h-9 w-12 cursor-pointer rounded-lg border border-input"
										/>
										<input
											type="text"
											bind:value={embedForm.primary_color}
											class="flex-1 rounded-lg border border-input bg-background px-3 py-2 text-sm font-mono outline-none ring-ring focus:ring-2"
										/>
									</div>
								</div>
							</div>

							<div class="space-y-1.5">
								<label for="embedGreeting" class="text-sm font-medium">Greeting Message</label>
								<input
									id="embedGreeting"
									type="text"
									bind:value={embedForm.greeting_message}
									class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
								/>
							</div>

							<div class="space-y-1.5">
								<label for="embedRate" class="text-sm font-medium">Rate Limit (messages/session)</label>
								<input
									id="embedRate"
									type="number"
									min="1"
									bind:value={embedForm.rate_limit}
									class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
								/>
							</div>

							<div class="space-y-1.5">
								<label for="embedSystemPrompt" class="text-sm font-medium">System Prompt</label>
								<textarea
									id="embedSystemPrompt"
									bind:value={embedForm.system_prompt}
									rows="3"
									placeholder="Custom system prompt for this widget (optional)"
									class="w-full rounded-lg border border-input bg-background px-3 py-2.5 text-sm outline-none ring-ring focus:ring-2"
								></textarea>
							</div>

							<div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
								<div class="space-y-1.5">
									<label for="embedProvider" class="text-sm font-medium">LLM Provider</label>
									<input
										id="embedProvider"
										type="text"
										bind:value={embedForm.provider}
										placeholder="e.g. openai (optional, uses default)"
										class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
									/>
								</div>
								<div class="space-y-1.5">
									<label for="embedModel" class="text-sm font-medium">Model</label>
									<input
										id="embedModel"
										type="text"
										bind:value={embedForm.model}
										placeholder="e.g. gpt-4o (optional, uses default)"
										class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
									/>
								</div>
							</div>

							<div class="space-y-1.5">
								<label for="embedApiKey" class="text-sm font-medium">
									API Key {editingEmbedId ? '(leave empty to keep current)' : ''}
								</label>
								<input
									id="embedApiKey"
									type="password"
									bind:value={embedForm.api_key}
									placeholder="sk-..."
									class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
								/>
								<p class="text-xs text-muted-foreground">
									Dedicated API key for this widget. If empty, the system default will be used.
								</p>
							</div>

							<div class="flex justify-end border-t border-border pt-4">
								<button
									onclick={saveEmbedKey}
									disabled={embedSaving || !embedForm.name.trim()}
									class="rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
								>
									{embedSaving ? 'Saving...' : editingEmbedId ? 'Update Key' : 'Create Key'}
								</button>
							</div>
						</div>
					</section>
				{:else}
					<!-- Embed keys list -->
					<section class="space-y-4">
						<div class="flex items-center justify-between">
							<div>
								<h2 class="text-base font-semibold">Embeddable Widgets</h2>
								<p class="text-xs text-muted-foreground">
									Create embed keys to add chat widgets to external websites.
								</p>
							</div>
							<button
								onclick={() => (showEmbedForm = true)}
								class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
							>
								Create Key
							</button>
						</div>

						{#if embedKeys.length === 0}
							<div class="rounded-xl border border-dashed border-border py-12 text-center">
								<p class="text-sm text-muted-foreground">No embed keys yet.</p>
								<p class="mt-1 text-xs text-muted-foreground">
									Create one to embed a chat widget on your website.
								</p>
							</div>
						{:else}
							<div class="space-y-3">
								{#each embedKeys as key}
									<div class="rounded-xl border border-border bg-card p-4 space-y-3">
										<div class="flex items-center justify-between">
											<div>
												<div class="flex items-center gap-2">
													<p class="text-sm font-semibold">{key.name}</p>
													<span
														class="rounded-full px-2 py-0.5 text-xs font-medium {key.is_active
															? 'bg-success/10 text-success'
															: 'bg-destructive/10 text-destructive'}"
													>
														{key.is_active ? 'Active' : 'Inactive'}
													</span>
												</div>
												<p class="text-xs text-muted-foreground">
													{key.key_prefix}... &middot; Created {formatDate(key.created_at)}
												</p>
											</div>
											<div class="flex items-center gap-1">
												<button
													onclick={() => toggleEmbedKey(key.id)}
													class="rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-accent"
												>
													{key.is_active ? 'Disable' : 'Enable'}
												</button>
												<button
													onclick={() => startEditEmbed(key)}
													class="rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-accent"
												>
													Edit
												</button>
												<button
													onclick={() => deleteEmbedKey(key.id, key.name)}
													class="rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
												>
													Delete
												</button>
											</div>
										</div>

										<!-- Stats -->
										<div class="flex gap-6 text-xs text-muted-foreground">
											<span>{key.total_conversations} conversations</span>
											<span>{key.total_messages} messages</span>
											<span>Rate limit: {key.rate_limit}/session</span>
											{#if key.allowed_domains.length > 0}
												<span>Domains: {key.allowed_domains.join(', ')}</span>
											{:else}
												<span>All domains</span>
											{/if}
										</div>

										<!-- Embed snippet -->
										<div class="space-y-1.5">
											<p class="text-xs font-medium text-muted-foreground">Embed Code</p>
											<div class="flex items-center gap-2">
												<code
													class="flex-1 rounded-lg border border-input bg-background px-3 py-2 text-xs font-mono break-all"
												>
													{getEmbedSnippet(key)}
												</code>
												<button
													onclick={() => copyToClipboard(getEmbedSnippet(key), key.id)}
													class="shrink-0 rounded-md border border-input px-2 py-1 text-xs hover:bg-accent"
												>
													{copiedSnippetId === key.id ? 'Copied!' : 'Copy'}
												</button>
											</div>
										</div>
									</div>
								{/each}
							</div>
						{/if}
					</section>
				{/if}
			{/if}
		</div>
	</div>
</div>
