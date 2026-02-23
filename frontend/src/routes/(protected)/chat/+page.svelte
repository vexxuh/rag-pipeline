<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$api/client';
	import { renderMarkdown } from '$lib/markdown';
	import type { Conversation, ConversationWithMessages, Message } from '$types/index';

	let conversations: Conversation[] = $state([]);
	let activeConversationId: string | null = $state(null);
	let messages: Message[] = $state([]);
	let input = $state('');
	let streaming = $state(false);
	let loading = $state(false);
	let messagesContainer: HTMLElement | undefined = $state();

	onMount(async () => {
		await loadConversations();
	});

	function scrollToBottom() {
		if (messagesContainer) {
			messagesContainer.scrollTop = messagesContainer.scrollHeight;
		}
	}

	async function loadConversations() {
		try {
			conversations = await api.get<Conversation[]>('/api/conversations');
		} catch {
			// empty
		}
	}

	async function createConversation() {
		try {
			const conv = await api.post<Conversation>('/api/conversations', {});
			conversations = [conv, ...conversations];
			await selectConversation(conv.id);
		} catch {
			// empty
		}
	}

	async function selectConversation(id: string) {
		if (id === activeConversationId) return;

		loading = true;
		activeConversationId = id;
		messages = [];

		try {
			const data = await api.get<ConversationWithMessages>(`/api/conversations/${id}`);
			messages = data.messages;
			setTimeout(scrollToBottom, 50);
		} catch {
			// empty
		} finally {
			loading = false;
		}
	}

	async function deleteConversation(id: string) {
		try {
			await api.delete(`/api/conversations/${id}`);
			conversations = conversations.filter((c) => c.id !== id);
			if (activeConversationId === id) {
				activeConversationId = null;
				messages = [];
			}
		} catch {
			// empty
		}
	}

	async function sendMessage() {
		const text = input.trim();
		if (!text || streaming) return;

		// Create conversation on first message if none selected
		if (!activeConversationId) {
			try {
				const conv = await api.post<Conversation>('/api/conversations', {});
				conversations = [conv, ...conversations];
				activeConversationId = conv.id;
			} catch {
				return;
			}
		}

		const userMsg: Message = {
			id: crypto.randomUUID(),
			conversation_id: activeConversationId!,
			role: 'user',
			content: text,
			created_at: new Date().toISOString()
		};

		messages = [...messages, userMsg];
		input = '';
		streaming = true;

		const assistantMsg: Message = {
			id: crypto.randomUUID(),
			conversation_id: activeConversationId!,
			role: 'assistant',
			content: '',
			created_at: new Date().toISOString()
		};
		messages = [...messages, assistantMsg];

		try {
			for await (const chunk of api.stream(
				`/api/conversations/${activeConversationId}/messages`,
				{ message: text }
			)) {
				assistantMsg.content += chunk;
				messages = [...messages.slice(0, -1), { ...assistantMsg }];
				scrollToBottom();
			}

			// Refresh conversation list (title may have changed)
			await loadConversations();
		} catch (e) {
			assistantMsg.content =
				'Error: ' + (e instanceof Error ? e.message : 'Failed to get response');
			messages = [...messages.slice(0, -1), { ...assistantMsg }];
		} finally {
			streaming = false;
			scrollToBottom();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			sendMessage();
		}
	}

	function formatTime(dateStr: string): string {
		const d = new Date(dateStr);
		const now = new Date();
		const diffMs = now.getTime() - d.getTime();
		const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

		if (diffDays === 0) return 'Today';
		if (diffDays === 1) return 'Yesterday';
		if (diffDays < 7) return `${diffDays}d ago`;
		return d.toLocaleDateString();
	}
</script>

<div class="flex h-full">
	<!-- Conversation sidebar -->
	<div class="flex w-64 flex-col border-r border-border bg-card/50">
		<div class="flex items-center justify-between border-b border-border px-4 py-3">
			<h2 class="text-sm font-semibold">Chats</h2>
			<button
				onclick={createConversation}
				class="rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90"
			>
				+ New
			</button>
		</div>

		<div class="flex-1 overflow-y-auto">
			{#if conversations.length === 0}
				<p class="px-4 py-6 text-center text-xs text-muted-foreground">
					No conversations yet
				</p>
			{:else}
				<div class="space-y-0.5 p-2">
					{#each conversations as conv}
						<div
							class="group flex items-center rounded-lg transition-colors {conv.id === activeConversationId
								? 'bg-accent text-accent-foreground'
								: 'text-muted-foreground hover:bg-accent/50'}"
						>
							<button
								onclick={() => selectConversation(conv.id)}
								class="flex-1 min-w-0 px-3 py-2 text-left"
							>
								<p class="truncate text-sm">{conv.title}</p>
								<p class="text-xs opacity-60">{formatTime(conv.updated_at)}</p>
							</button>
							<button
								onclick={() => deleteConversation(conv.id)}
								class="mr-2 shrink-0 rounded p-1 text-xs opacity-0 hover:bg-destructive/10 hover:text-destructive group-hover:opacity-100"
								title="Delete conversation"
							>
								&times;
							</button>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</div>

	<!-- Chat area -->
	<div class="flex flex-1 flex-col">
		<!-- Messages -->
		<div class="flex-1 overflow-y-auto" bind:this={messagesContainer}>
			{#if !activeConversationId && messages.length === 0}
				<div class="flex h-full flex-col items-center justify-center gap-4 text-center">
					<div
						class="flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10 text-2xl font-bold text-primary"
					>
						R
					</div>
					<div>
						<h2 class="text-xl font-semibold">RAG Pipeline</h2>
						<p class="mt-1 text-sm text-muted-foreground">
							Ask questions about your documents and web content.
						</p>
						<p class="mt-3 text-xs text-muted-foreground">
							Start typing below or click <strong>+ New</strong> to begin a conversation.
						</p>
					</div>
				</div>
			{:else if loading}
				<div class="flex h-full items-center justify-center">
					<div class="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent"></div>
				</div>
			{:else}
				<div class="mx-auto max-w-3xl space-y-6 p-6">
					{#each messages as msg}
						<div class="flex gap-3 {msg.role === 'user' ? 'flex-row-reverse' : ''}">
							<div
								class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-xs font-bold {msg.role === 'user'
									? 'bg-primary text-primary-foreground'
									: 'bg-secondary text-secondary-foreground'}"
							>
								{msg.role === 'user' ? 'U' : 'AI'}
							</div>
							<div
								class="max-w-[80%] rounded-2xl px-4 py-3 text-sm leading-relaxed {msg.role === 'user'
									? 'bg-primary text-primary-foreground'
									: 'bg-card border border-border'}"
							>
								{#if msg.role === 'assistant'}
									<div class="prose prose-sm dark:prose-invert max-w-none">
										{@html renderMarkdown(msg.content)}
									</div>
									{#if streaming && msg === messages[messages.length - 1] && !msg.content}
										<span class="inline-block h-4 w-1 animate-pulse bg-current"></span>
									{/if}
								{:else}
									<p class="whitespace-pre-wrap">{msg.content}</p>
								{/if}
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Input -->
		<div class="border-t border-border p-4">
			<div class="mx-auto max-w-3xl">
				<div class="flex items-end gap-2 rounded-xl border border-border bg-card p-2">
					<textarea
						bind:value={input}
						onkeydown={handleKeydown}
						placeholder="Ask a question about your documents..."
						disabled={streaming}
						rows="1"
						class="flex-1 resize-none bg-transparent px-2 py-1.5 text-sm outline-none placeholder:text-muted-foreground"
					></textarea>
					<button
						onclick={sendMessage}
						disabled={!input.trim() || streaming}
						class="shrink-0 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
					>
						{streaming ? 'Sending...' : 'Send'}
					</button>
				</div>
				<p class="mt-2 text-center text-xs text-muted-foreground">
					Configure your LLM provider and API keys in
					<a href="/settings" class="underline">Settings</a>
				</p>
			</div>
		</div>
	</div>
</div>
