<script lang="ts">
	// The areas, from the registry, a line between its groups; a count
	// where the area has one; the links keep the range, and the help ends
	// the panel's own group. `compact`: icons
	// only, for the collapsed sidebar.
	import { page } from '$app/state';
	import KeyboardIcon from '@lucide/svelte/icons/keyboard';
	import { help } from '#lib/help.svelte.js';
	import { offered } from '#lib/nav.js';
	import { live } from '#lib/live/store.svelte.js';
	import { scope } from '#lib/scope.svelte.js';
	import { cn } from '#lib/utils.js';

	let { onnavigate, compact = false }: { onnavigate?: () => void; compact?: boolean } = $props();

	const inScope = $derived(live.list.filter((s) => scope.covers(s)));
	const range = $derived(page.url.searchParams.get('range'));
</script>

<nav class="flex flex-col gap-0.5 p-2" aria-label="Areas">
	{#each offered() as area, i (area.href)}
		{@const active = area.match(page.url.pathname)}
		{#if i > 0 && offered()[i - 1].group !== area.group}
			<hr class="my-1.5 border-sidebar-border" />
		{/if}
		{@const badge = area.badge?.(inScope)}
		<a
			href={range ? `${area.href}?range=${encodeURIComponent(range)}` : area.href}
			aria-current={active ? 'page' : undefined}
			title={compact ? (badge ? `${area.label} (${badge})` : area.label) : undefined}
			class={cn(
				'relative flex items-center gap-2.5 rounded-md py-2 text-sm hover:bg-sidebar-accent hover:text-sidebar-accent-foreground',
				compact ? 'justify-center px-2' : 'px-2.5',
				active && 'bg-sidebar-accent font-medium text-sidebar-accent-foreground'
			)}
			onclick={onnavigate}
		>
			<area.icon class="size-4 shrink-0 opacity-80" />
			{#if compact}
				<span class="sr-only">{area.label}</span>
				{#if badge}
					<span
						class="absolute top-0.5 right-0.5 min-w-4 rounded-full bg-muted px-1 text-center text-[10px] leading-4 text-muted-foreground tabular-nums"
						>{badge}</span
					>
				{/if}
			{:else}
				<span class="flex-1">{area.label}</span>
				{#if badge}
					<span class="rounded-full bg-muted px-1.5 text-xs text-muted-foreground tabular-nums"
						>{badge}</span
					>
				{/if}
			{/if}
		</a>
	{/each}
	<button
		type="button"
		title={compact ? 'Keys and help (?)' : undefined}
		class={cn(
			'flex items-center gap-2.5 rounded-md py-2 text-sm text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground',
			compact ? 'justify-center px-2' : 'px-2.5'
		)}
		onclick={() => {
			onnavigate?.();
			help.show();
		}}
	>
		<KeyboardIcon class="size-4 shrink-0 opacity-80" />
		{#if compact}
			<span class="sr-only">Keys and help</span>
		{:else}
			<span class="flex-1 text-left">Keys and help</span>
			<kbd class="hidden font-mono text-xs md:inline">?</kbd>
		{/if}
	</button>
</nav>
