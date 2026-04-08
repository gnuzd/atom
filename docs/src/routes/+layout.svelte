<script lang="ts">
	import '../app.css';
	import { page } from '$app/stores';
	import { LayoutGrid, BookOpen, Download, Terminal } from 'lucide-svelte';

	let { children } = $props();

	const navItems = [
		{ name: 'Introduction', path: '/', icon: BookOpen },
		{ name: 'Prerequisites', path: '/prerequisites', icon: Terminal },
		{ name: 'Installation', path: '/installation', icon: Download }
	];
</script>

<div class="flex h-screen overflow-hidden">
	<!-- Sidebar -->
	<aside class="w-64 bg-tokyo-sidebar border-r border-tokyo-border flex flex-col">
		<div class="p-6 border-b border-tokyo-border">
			<h1 class="text-xl font-bold text-tokyo-blue flex items-center gap-2">
				<LayoutGrid size={24} />
				Atom Docs
			</h1>
		</div>
		<nav class="flex-1 p-4 overflow-y-auto">
			<ul class="space-y-2">
				{#each navItems as item}
					<li>
						<a
							href={item.path}
							class="flex items-center gap-3 px-4 py-2 rounded-lg transition-colors { $page.url.pathname === item.path ? 'bg-tokyo-border text-tokyo-blue' : 'text-tokyo-muted hover:bg-tokyo-bg hover:text-tokyo-text' }"
						>
							<item.icon size={18} />
							{item.name}
						</a>
					</li>
				{/each}
			</ul>
		</nav>
		<div class="p-4 border-t border-tokyo-border text-xs text-tokyo-muted">
			Built with SvelteKit & ❤️
		</div>
	</aside>

	<!-- Main Content -->
	<main class="flex-1 overflow-y-auto p-8 lg:p-12">
		<div class="max-w-4xl mx-auto prose prose-invert lg:prose-lg">
			{@render children()}
		</div>
	</main>
</div>
