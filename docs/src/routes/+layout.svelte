<script lang="ts">
	import '../app.css';
	import { page } from '$app/stores';
	import { afterNavigate } from '$app/navigation';
	import { LayoutGrid, BookOpen, Download, Terminal, Menu, X } from 'lucide-svelte';

	let { children } = $props();
	let isSidebarOpen = $state(false);

	const navItems = [
		{ name: 'Introduction', path: '/', icon: BookOpen },
		{ name: 'Prerequisites', path: '/prerequisites', icon: Terminal },
		{ name: 'Installation', path: '/installation', icon: Download }
	];

	function toggleSidebar() {
		isSidebarOpen = !isSidebarOpen;
	}

	afterNavigate(() => {
		isSidebarOpen = false;
	});
</script>

<div class="flex h-screen overflow-hidden bg-tokyo-bg text-tokyo-text font-sans selection:bg-tokyo-blue/30">
	<!-- Mobile Header -->
	<header class="lg:hidden fixed top-0 left-0 right-0 h-16 bg-tokyo-sidebar/80 backdrop-blur-md border-b border-tokyo-border z-40 flex items-center justify-between px-6">
		<a href="/" class="text-xl font-bold text-tokyo-blue flex items-center gap-2">
			<LayoutGrid size={24} />
			Atom Docs
		</a>
		<button 
			onclick={toggleSidebar}
			class="p-2 text-tokyo-text hover:text-tokyo-blue transition-colors"
			aria-label="Toggle Menu"
		>
			{#if isSidebarOpen}
				<X size={24} />
			{:else}
				<Menu size={24} />
			{/if}
		</button>
	</header>

	<!-- Sidebar / Mobile Menu overlay -->
	<aside 
		class="fixed inset-y-0 left-0 w-72 bg-tokyo-sidebar border-r border-tokyo-border flex flex-col z-50 transition-transform duration-300 ease-in-out lg:relative lg:translate-x-0 {isSidebarOpen ? 'translate-x-0' : '-translate-x-full'}"
	>
		<div class="p-8 border-b border-tokyo-border flex items-center justify-between lg:block">
			<h1 class="text-2xl font-black text-tokyo-blue flex items-center gap-3 tracking-tight">
				<LayoutGrid size={28} class="text-tokyo-purple" />
				Atom Docs
			</h1>
			<button onclick={toggleSidebar} class="lg:hidden p-2 text-tokyo-muted hover:text-tokyo-text">
				<X size={24} />
			</button>
		</div>

		<nav class="flex-1 p-6 overflow-y-auto space-y-8">
			<div>
				<h2 class="text-xs font-semibold text-tokyo-muted uppercase tracking-widest mb-4 px-4">Documentation</h2>
				<ul class="space-y-1">
					{#each navItems as item}
						<li>
							<a
								href={item.path}
								class="flex items-center gap-3 px-4 py-2.5 rounded-xl font-medium transition-all duration-200 { $page.url.pathname === item.path ? 'bg-tokyo-blue/10 text-tokyo-blue ring-1 ring-tokyo-blue/20' : 'text-tokyo-muted hover:bg-tokyo-border/50 hover:text-tokyo-text' }"
							>
								<item.icon size={18} />
								{item.name}
							</a>
						</li>
					{/each}
				</ul>
			</div>

			<div class="pt-4 border-t border-tokyo-border">
				<h2 class="text-xs font-semibold text-tokyo-muted uppercase tracking-widest mb-4 px-4">Community</h2>
				<ul class="space-y-1">
					<li>
						<a 
							href="https://github.com/gnuzd/atom" 
							target="_blank"
							class="flex items-center gap-3 px-4 py-2.5 rounded-xl font-medium text-tokyo-muted hover:bg-tokyo-border/50 hover:text-tokyo-text transition-all duration-200"
						>
							<svg viewBox="0 0 24 24" class="w-[18px] h-[18px] fill-current" xmlns="http://www.w3.org/2000/svg"><path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12"/></svg>
							GitHub Repository
						</a>
					</li>
				</ul>
			</div>
		</nav>

		<div class="p-6 border-t border-tokyo-border flex items-center gap-3">
			<div class="w-8 h-8 rounded-full bg-tokyo-purple/20 flex items-center justify-center text-tokyo-purple font-bold text-sm">A</div>
			<div class="flex flex-col">
				<span class="text-xs font-semibold text-tokyo-text">Atom v0.1.0</span>
				<span class="text-[10px] text-tokyo-muted">MIT Licensed</span>
			</div>
		</div>
	</aside>

	<!-- Mobile Menu Background Overlay -->
	{#if isSidebarOpen}
		<button 
			onclick={toggleSidebar}
			class="fixed inset-0 bg-black/50 backdrop-blur-sm z-45 lg:hidden"
			aria-label="Close Sidebar"
		></button>
	{/if}

	<!-- Main Content -->
	<main class="flex-1 overflow-y-auto pt-16 lg:pt-0">
		<div class="max-w-4xl mx-auto px-6 py-12 lg:px-12 lg:py-16">
			<article class="prose prose-invert lg:prose-lg prose-tokyo max-w-none">
				{@render children()}
			</article>

			<!-- Simple Footer -->
			<footer class="mt-16 pt-8 border-t border-tokyo-border flex flex-col sm:flex-row justify-between items-center gap-4 text-sm text-tokyo-muted">
				<p>© 2026 Atom Editor Team</p>
				<div class="flex gap-6">
					<a href="https://github.com/gnuzd/atom" class="hover:text-tokyo-blue transition-colors">GitHub</a>
					<a href="/installation" class="hover:text-tokyo-blue transition-colors">Install</a>
					<a href="/prerequisites" class="hover:text-tokyo-blue transition-colors">Setup</a>
				</div>
			</footer>
		</div>
	</main>
</div>

<style>
	:global(html) {
		scrollbar-gutter: stable;
	}
	
	/* Custom scrollbar for a more integrated look */
	::-webkit-scrollbar {
		width: 8px;
		height: 8px;
	}
	::-webkit-scrollbar-track {
		background: transparent;
	}
	::-webkit-scrollbar-thumb {
		background: #24283b;
		border-radius: 10px;
	}
	::-webkit-scrollbar-thumb:hover {
		background: #414868;
	}
</style>
