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
	<aside class="w-64 bg-[#16161e] border-r border-[#24283b] flex flex-col">
		<div class="p-6 border-b border-[#24283b]">
			<h1 class="text-xl font-bold text-[#7aa2f7] flex items-center gap-2">
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
							class="flex items-center gap-3 px-4 py-2 rounded-lg transition-colors { $page.url.pathname === item.path ? 'bg-[#24283b] text-[#7aa2f7]' : 'text-[#565f89] hover:bg-[#1a1b26] hover:text-[#c0caf5]' }"
						>
							<item.icon size={18} />
							{item.name}
						</a>
					</li>
				{/each}
			</ul>
		</nav>
		<div class="p-4 border-t border-[#24283b] text-xs text-[#565f89]">
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
