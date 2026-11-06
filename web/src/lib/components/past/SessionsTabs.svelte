<script lang="ts">
	// Sessions › Open (what runs) and Past (every conversation on the hub).
	import { page } from '$app/state';
	import { live } from '#lib/live/store.svelte.js';
	import { PAST } from '#lib/past.js';
	import { scope } from '#lib/scope.svelte.js';

	const past = $derived(page.url.pathname === PAST);
	const open = $derived(live.list.filter((s) => s.activity !== 'gone' && scope.covers(s)).length);
	const tab = 'border-b-2 px-3 py-1.5 -mb-px';
</script>

<nav class="flex gap-1 border-b text-sm" aria-label="Sessions">
	<a
		href="/sessions"
		aria-current={past ? undefined : 'page'}
		class="{tab} {past
			? 'border-transparent text-muted-foreground hover:text-foreground'
			: 'border-primary font-medium'}"
		>Open{#if open}<span class="ml-1 text-xs text-muted-foreground">{open}</span>{/if}</a
	>
	<a
		href={PAST}
		aria-current={past ? 'page' : undefined}
		class="{tab} {past
			? 'border-primary font-medium'
			: 'border-transparent text-muted-foreground hover:text-foreground'}">Past</a
	>
</nav>
