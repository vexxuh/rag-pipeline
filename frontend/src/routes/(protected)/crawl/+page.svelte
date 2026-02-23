<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$api/client';
	import type { CrawlJob } from '$types/index';

	let jobs: CrawlJob[] = $state([]);
	let url = $state('');
	let crawlType: 'sitemap' | 'full' = $state('sitemap');
	let loading = $state(false);
	let error = $state('');

	onMount(async () => {
		await loadJobs();
	});

	async function loadJobs() {
		try {
			jobs = await api.get<CrawlJob[]>('/api/crawl');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load crawl jobs';
		}
	}

	async function startCrawl() {
		if (!url.trim()) return;

		error = '';
		loading = true;

		try {
			await api.post('/api/crawl', { url: url.trim(), crawl_type: crawlType });
			url = '';
			await loadJobs();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to start crawl';
		} finally {
			loading = false;
		}
	}

	function statusColor(status: string): string {
		switch (status) {
			case 'completed':
				return 'bg-success/10 text-success';
			case 'running':
				return 'bg-warning/10 text-warning';
			case 'failed':
				return 'bg-destructive/10 text-destructive';
			default:
				return 'bg-muted text-muted-foreground';
		}
	}

	function formatDate(dateStr: string | null): string {
		if (!dateStr) return '-';
		return new Date(dateStr).toLocaleString();
	}
</script>

<div class="flex h-full flex-col">
	<div class="border-b border-border px-6 py-4">
		<h1 class="text-lg font-semibold">Web Crawl</h1>
		<p class="text-sm text-muted-foreground">Crawl websites to add content to the RAG pipeline</p>
	</div>

	<div class="flex-1 overflow-y-auto p-6">
		<div class="mx-auto max-w-3xl space-y-6">
			{#if error}
				<div class="rounded-lg bg-destructive/10 px-4 py-3 text-sm text-destructive">
					{error}
					<button onclick={() => (error = '')} class="ml-2 underline">Dismiss</button>
				</div>
			{/if}

			<!-- Start crawl form -->
			<div class="rounded-xl border border-border bg-card p-6 space-y-4">
				<h2 class="font-medium text-sm">Start New Crawl</h2>

				<div class="space-y-2">
					<label for="url" class="text-sm text-muted-foreground">Website URL</label>
					<input
						id="url"
						type="url"
						bind:value={url}
						placeholder="https://example.com"
						class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none ring-ring focus:ring-2"
					/>
				</div>

				<div class="flex gap-4">
					<label class="flex items-center gap-2 text-sm">
						<input type="radio" bind:group={crawlType} value="sitemap" class="accent-primary" />
						Sitemap (faster)
					</label>
					<label class="flex items-center gap-2 text-sm">
						<input type="radio" bind:group={crawlType} value="full" class="accent-primary" />
						Full crawl
					</label>
				</div>

				<button
					onclick={startCrawl}
					disabled={loading || !url.trim()}
					class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
				>
					{loading ? 'Starting...' : 'Start Crawl'}
				</button>
			</div>

			<!-- Jobs list -->
			<div class="space-y-3">
				<div class="flex items-center justify-between">
					<h2 class="font-medium text-sm">Crawl Jobs</h2>
					<button
						onclick={loadJobs}
						class="rounded-md px-3 py-1.5 text-xs text-muted-foreground hover:bg-accent"
					>
						Refresh
					</button>
				</div>

				{#if jobs.length === 0}
					<p class="py-12 text-center text-sm text-muted-foreground">
						No crawl jobs yet.
					</p>
				{:else}
					{#each jobs as job}
						<div class="rounded-xl border border-border bg-card p-4 space-y-2">
							<div class="flex items-center justify-between">
								<p class="truncate font-medium text-sm">{job.url}</p>
								<span class="shrink-0 rounded-full px-2 py-0.5 text-xs {statusColor(job.status)}">
									{job.status}
								</span>
							</div>
							<div class="flex items-center gap-4 text-xs text-muted-foreground">
								<span>Type: {job.crawl_type}</span>
								<span>Pages: {job.pages_processed}/{job.pages_found}</span>
								<span>Started: {formatDate(job.started_at)}</span>
							</div>
							{#if job.error_message}
								<p class="text-xs text-destructive">{job.error_message}</p>
							{/if}
						</div>
					{/each}
				{/if}
			</div>
		</div>
	</div>
</div>
