<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$api/client';
	import type { ApiKey, LlmPreferences, AdminProvider, AdminModel } from '$types/index';

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
	let error = $state('');
	let success = $state('');
	let saving = $state(false);

	onMount(async () => {
		await Promise.all([loadProviders(), loadApiKeys(), loadPreferences()]);
		// Load models for the user's current preferred provider
		if (preferences.preferred_provider) {
			await loadModelsForProvider(preferences.preferred_provider);
		}
	});

	async function loadProviders() {
		try {
			providers = await api.get<AdminProvider[]>('/api/settings/providers');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load providers';
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

	async function loadApiKeys() {
		try {
			apiKeys = await api.get<ApiKey[]>('/api/settings/api-keys');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load API keys';
		}
	}

	async function loadPreferences() {
		try {
			const prefs = await api.get<LlmPreferences | null>('/api/settings/preferences');
			if (prefs) {
				preferences = prefs;
			}
		} catch {
			// Use defaults
		}
	}

	async function onProviderChange() {
		if (preferences.preferred_provider) {
			await loadModelsForProvider(preferences.preferred_provider);

			// Auto-select default models or first available
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
			await loadApiKeys();
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
</script>

<div class="flex h-full flex-col">
	<div class="border-b border-border px-6 py-4">
		<h1 class="text-lg font-semibold">Settings</h1>
		<p class="text-sm text-muted-foreground">Configure LLM providers and preferences</p>
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
					<!-- Provider -->
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

					<!-- Models row -->
					<div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
						<!-- Completion Model -->
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

						<!-- Embedding Model -->
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

					<!-- System Prompt -->
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
		</div>
	</div>
</div>
