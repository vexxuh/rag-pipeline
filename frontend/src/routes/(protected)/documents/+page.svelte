<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$api/client';
	import type { Document } from '$types/index';

	let documents: Document[] = $state([]);
	let uploading = $state(false);
	let rescanning = $state(false);
	let success = $state('');
	let error = $state('');
	let fileInput: HTMLInputElement | undefined = $state();
	let maxUploadSizeMb = $state(50);

	onMount(async () => {
		await loadDocuments();
		try {
			const limits = await api.get<{ max_upload_size_mb: number }>('/api/documents/limits');
			maxUploadSizeMb = limits.max_upload_size_mb;
		} catch {
			// fallback to default
		}
	});

	async function loadDocuments() {
		try {
			documents = await api.get<Document[]>('/api/documents');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load documents';
		}
	}

	const SUPPORTED_EXTENSIONS = ['.pdf', '.docx', '.xlsx', '.xls', '.xml', '.csv', '.txt', '.md'];

	async function handleUpload() {
		const file = fileInput?.files?.[0];
		if (!file) return;

		const ext = '.' + file.name.split('.').pop()?.toLowerCase();
		if (!SUPPORTED_EXTENSIONS.includes(ext)) {
			error = 'Unsupported file type. Supported: PDF, DOCX, XLSX, XML, CSV, TXT, MD';
			return;
		}

		const maxBytes = maxUploadSizeMb * 1024 * 1024;
		if (file.size > maxBytes) {
			error = `File too large (${(file.size / 1024 / 1024).toFixed(1)} MB). Maximum upload size is ${maxUploadSizeMb} MB.`;
			if (fileInput) fileInput.value = '';
			return;
		}

		error = '';
		uploading = true;

		try {
			await api.upload<Document>('/api/documents', file);
			await loadDocuments();
			if (fileInput) fileInput.value = '';
		} catch (e) {
			error = e instanceof Error ? e.message : 'Upload failed';
		} finally {
			uploading = false;
		}
	}

	async function rescanDocuments() {
		rescanning = true;
		error = '';
		try {
			const resp = await api.post<{ message: string }>('/api/documents/rescan');
			success = resp.message;
			setTimeout(() => (success = ''), 5000);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Rescan failed';
		} finally {
			rescanning = false;
		}
	}

	async function deleteDocument(id: string) {
		try {
			await api.delete(`/api/documents/${id}`);
			documents = documents.filter((d) => d.id !== id);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Delete failed';
		}
	}

	function formatBytes(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	}

	function statusColor(status: string): string {
		switch (status) {
			case 'ready':
				return 'bg-success/10 text-success';
			case 'processing':
				return 'bg-warning/10 text-warning';
			case 'failed':
				return 'bg-destructive/10 text-destructive';
			default:
				return 'bg-muted text-muted-foreground';
		}
	}
</script>

<div class="flex h-full flex-col">
	<div class="border-b border-border px-6 py-4">
		<h1 class="text-lg font-semibold">Documents</h1>
		<p class="text-sm text-muted-foreground">Upload documents to add context for the RAG pipeline</p>
	</div>

	<div class="flex-1 overflow-y-auto p-6">
		<div class="mx-auto max-w-3xl space-y-6">
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

			<!-- Upload section -->
			<div class="rounded-xl border border-dashed border-border p-6 text-center">
				<p class="mb-3 text-sm text-muted-foreground">Upload a document (PDF, DOCX, XLSX, XML, CSV, TXT, MD — max {maxUploadSizeMb} MB)</p>
				<div class="flex items-center justify-center gap-3">
					<input
						bind:this={fileInput}
						type="file"
						accept=".pdf,.docx,.xlsx,.xls,.xml,.csv,.txt,.md"
						onchange={handleUpload}
						disabled={uploading}
						class="text-sm file:mr-3 file:rounded-lg file:border-0 file:bg-primary file:px-4 file:py-2 file:text-sm file:font-medium file:text-primary-foreground hover:file:bg-primary/90"
					/>
				</div>
				{#if uploading}
					<p class="mt-3 text-sm text-muted-foreground">Uploading...</p>
				{/if}
			</div>

			<!-- Documents list -->
			{#if documents.length > 0}
				<div class="flex items-center justify-between">
					<h2 class="text-sm font-medium text-muted-foreground">
						{documents.length} document{documents.length !== 1 ? 's' : ''}
					</h2>
					<button
						onclick={rescanDocuments}
						disabled={rescanning}
						class="rounded-lg border border-input px-4 py-2 text-sm font-medium hover:bg-accent disabled:opacity-50"
					>
						{rescanning ? 'Re-vectorizing...' : 'Re-vectorize All'}
					</button>
				</div>
			{/if}

			{#if documents.length === 0}
				<p class="py-12 text-center text-sm text-muted-foreground">
					No documents uploaded yet.
				</p>
			{:else}
				<div class="space-y-3">
					{#each documents as doc}
						<div
							class="flex items-center justify-between rounded-xl border border-border bg-card p-4"
						>
							<div class="min-w-0 flex-1">
								<p class="truncate font-medium text-sm">{doc.original_filename}</p>
								<div class="mt-1 flex items-center gap-3 text-xs text-muted-foreground">
									<span>{formatBytes(doc.size_bytes)}</span>
									<span class="rounded-full px-2 py-0.5 {statusColor(doc.status)}">
										{doc.status}
									</span>
									{#if doc.error_message}
										<span class="text-destructive">{doc.error_message}</span>
									{/if}
								</div>
							</div>
							<button
								onclick={() => deleteDocument(doc.id)}
								class="ml-4 shrink-0 rounded-md px-3 py-1.5 text-xs text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
							>
								Delete
							</button>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>
