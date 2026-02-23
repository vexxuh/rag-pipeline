<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { api } from '$api/client';
	import { authStore } from '$stores/auth';
	import type { User } from '$types/index';

	let { children } = $props();
	let ready = $state(false);
	let sidebarOpen = $state(false);
	let currentUser: User | null = $state(null);

	const allNavItems = [
		{ href: '/chat', label: 'Chat', icon: 'C', minRole: 'user' as const },
		{ href: '/documents', label: 'Documents', icon: 'D', minRole: 'maintainer' as const },
		{ href: '/crawl', label: 'Web Crawl', icon: 'W', minRole: 'maintainer' as const }
	];

	const roleLevels: Record<string, number> = { user: 0, maintainer: 1, admin: 2 };

	let navItems = $derived(
		allNavItems.filter((item) => (roleLevels[currentUser?.role ?? ''] ?? -1) >= roleLevels[item.minRole])
	);

	onMount(() => {
		const unsub = authStore.subscribe((state) => {
			currentUser = state.user;
		});

		api.get<User>('/api/auth/me')
			.then((user) => {
				authStore.setUser(user);
				ready = true;
			})
			.catch(() => {
				goto('/login');
			});

		return () => unsub();
	});

	function handleLogout() {
		authStore.logout();
		goto('/login');
	}

	function isActive(href: string): boolean {
		const path = $page.url.pathname;
		if (href === '/') return path === '/';
		return path.startsWith(href);
	}
</script>

{#if ready}
	<div class="flex h-screen overflow-hidden">
		<!-- Sidebar -->
		<aside
			class="flex w-64 flex-col border-r border-border bg-card max-md:fixed max-md:inset-y-0 max-md:left-0 max-md:z-40 max-md:transform max-md:transition-transform {sidebarOpen
				? 'max-md:translate-x-0'
				: 'max-md:-translate-x-full'}"
		>
			<div class="flex h-14 items-center border-b border-border px-4">
				<a href="/" class="text-lg font-bold">RAG Pipeline</a>
			</div>

			<nav class="flex-1 space-y-1 p-3">
				{#each navItems as item}
					<a
						href={item.href}
						class="flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors {isActive(item.href)
							? 'bg-accent text-accent-foreground'
							: 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'}"
						onclick={() => (sidebarOpen = false)}
					>
						<span
							class="flex h-6 w-6 items-center justify-center rounded-md bg-secondary text-xs font-bold"
						>
							{item.icon}
						</span>
						{item.label}
					</a>
				{/each}

				{#if currentUser?.role === 'admin'}
					<a
						href="/admin"
						class="flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors {isActive('/admin')
							? 'bg-accent text-accent-foreground'
							: 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'}"
						onclick={() => (sidebarOpen = false)}
					>
						<span
							class="flex h-6 w-6 items-center justify-center rounded-md bg-destructive/20 text-xs font-bold text-destructive"
						>
							A
						</span>
						Admin
					</a>
				{/if}
			</nav>

			<div class="border-t border-border p-3">
				<div class="flex items-center justify-between rounded-lg px-3 py-2">
					<div class="min-w-0">
						<p class="truncate text-sm font-medium">{currentUser?.username ?? ''}</p>
						<p class="truncate text-xs text-muted-foreground">{currentUser?.email ?? ''}</p>
					</div>
					<button
						onclick={handleLogout}
						class="ml-2 shrink-0 rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
					>
						Logout
					</button>
				</div>
			</div>
		</aside>

		<!-- Mobile overlay -->
		{#if sidebarOpen}
			<button
				class="fixed inset-0 z-30 bg-black/50 md:hidden"
				onclick={() => (sidebarOpen = false)}
				aria-label="Close sidebar"
				title="Close sidebar"
			></button>
		{/if}

		<!-- Main content -->
		<main class="flex flex-1 flex-col overflow-hidden">
			<header class="flex h-14 items-center border-b border-border px-4 md:hidden">
				<button
					onclick={() => (sidebarOpen = !sidebarOpen)}
					class="rounded-md p-2 text-muted-foreground hover:bg-accent"
					aria-label="Toggle sidebar"
				>
					<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width="2"
							d="M4 6h16M4 12h16M4 18h16"
						/>
					</svg>
				</button>
				<span class="ml-3 font-bold">RAG Pipeline</span>
			</header>

			<div class="flex-1 overflow-auto">
				{@render children()}
			</div>
		</main>
	</div>
{:else}
	<div class="flex h-screen items-center justify-center">
		<div class="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent">
		</div>
	</div>
{/if}
