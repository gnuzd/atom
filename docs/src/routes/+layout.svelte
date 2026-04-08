<script lang="ts">
	import '../app.css';
	import { page } from '$app/stores';
	import { afterNavigate } from '$app/navigation';
	import { 
		Selection, 
		BookOpen, 
		DownloadSimple, 
		TerminalWindow, 
		List, 
		X, 
		GithubLogo, 
		Package 
	} from 'phosphor-svelte';

	let { children } = $props();
	let isSidebarOpen = $state(false);

	const navItems = [
		{ name: 'Introduction', path: '/', icon: BookOpen },
		{ name: 'Prerequisites', path: '/prerequisites', icon: TerminalWindow },
		{ name: 'Installation', path: '/installation', icon: DownloadSimple }
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
			<Selection size={24} weight="bold" />
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
				<List size={24} />
			{/if}
		</button>
	</header>

	<!-- Sidebar / Mobile Menu overlay -->
	<aside 
		class="fixed inset-y-0 left-0 w-72 bg-tokyo-sidebar border-r border-tokyo-border flex flex-col z-50 transition-transform duration-300 ease-in-out lg:relative lg:translate-x-0 {isSidebarOpen ? 'translate-x-0' : '-translate-x-full'}"
	>
		<div class="p-8 border-b border-tokyo-border flex items-center justify-between lg:block">
			<h1 class="text-2xl font-black text-tokyo-blue flex items-center gap-3 tracking-tight">
				<Selection size={28} weight="fill" class="text-tokyo-purple" />
				Atom Docs
			</h1>
			<button onclick={toggleSidebar} class="lg:hidden p-2 text-tokyo-muted hover:text-tokyo-text">
				<X size={24} />
			</button>
		</div>

		<nav class="flex-1 p-6 overflow-y-auto space-y-8">
			<div>
				<h2 class="text-[10px] font-bold text-tokyo-muted uppercase tracking-[0.2em] mb-4 px-4">Documentation</h2>
				<ul class="space-y-1">
					{#each navItems as item}
						{@const Icon = item.icon}
						<li>
							<a
								href={item.path}
								class="flex items-center gap-3 px-4 py-2.5 rounded-xl font-medium transition-all duration-200 { $page.url.pathname === item.path ? 'bg-tokyo-blue/10 text-tokyo-blue ring-1 ring-tokyo-blue/20' : 'text-tokyo-muted hover:bg-tokyo-border/50 hover:text-tokyo-text' }"
							>
								<Icon size={20} weight={$page.url.pathname === item.path ? 'fill' : 'regular'} />
								{item.name}
							</a>
						</li>
					{/each}
				</ul>
			</div>

			<div class="pt-4 border-t border-tokyo-border">
				<h2 class="text-[10px] font-bold text-tokyo-muted uppercase tracking-[0.2em] mb-4 px-4">Community</h2>
				<ul class="space-y-1">
					<li>
						<a 
							href="https://github.com/gnuzd/atom" 
							target="_blank"
							class="flex items-center gap-3 px-4 py-2.5 rounded-xl font-medium text-tokyo-muted hover:bg-tokyo-border/50 hover:text-tokyo-text transition-all duration-200"
						>
							<GithubLogo size={20} />
							GitHub Repository
						</a>
					</li>
				</ul>
			</div>
		</nav>

		<div class="p-6 border-t border-tokyo-border flex items-center gap-3 bg-tokyo-bg/30">
			<div class="w-8 h-8 rounded-lg bg-tokyo-purple/20 flex items-center justify-center text-tokyo-purple">
				<Package size={20} weight="fill" />
			</div>
			<div class="flex flex-col">
				<span class="text-xs font-bold text-tokyo-text">Atom v0.1.0</span>
				<span class="text-[10px] font-medium text-tokyo-muted">MIT Licensed</span>
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
			<footer class="mt-16 pt-8 border-t border-tokyo-border flex flex-col sm:flex-row justify-between items-center gap-4 text-xs font-medium text-tokyo-muted">
				<p>© 2026 Atom Editor Team</p>
				<div class="flex gap-8">
					<a href="https://github.com/gnuzd/atom" class="hover:text-tokyo-blue transition-colors flex items-center gap-2">
						<GithubLogo size={16} />
						GitHub
					</a>
					<a href="/installation" class="hover:text-tokyo-blue transition-colors flex items-center gap-2">
						<DownloadSimple size={16} />
						Install
					</a>
					<a href="/prerequisites" class="hover:text-tokyo-blue transition-colors flex items-center gap-2">
						<TerminalWindow size={16} />
						Setup
					</a>
				</div>
			</footer>
		</div>
	</main>
</div>

<style>
	:global(html) {
		scrollbar-gutter: stable;
	}
	
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
