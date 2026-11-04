<script lang="ts">
	// A report in a list: when, its title, where it belongs, its one line;
	// a link into the viewer.
	import type { Card } from '#lib/api/types.js';
	import { fmtStamp } from '#lib/format.js';
	import { line, reportHref } from '#lib/notes.js';
	import StatusBadge from '#lib/components/notes/StatusBadge.svelte';

	let {
		card,
		workspace = false,
		project = true,
		snippet
	}: { card: Card; workspace?: boolean; project?: boolean; snippet?: string } = $props();

	const where = $derived(
		[workspace ? card.workspace : null, project ? card.project : null].filter(Boolean).join(' / ')
	);
	const parts = $derived(snippet ? snippet.split(/\[([^\]]+)\]/) : null);
	const text = $derived(line(card));
</script>

<a
	class="flex flex-col gap-0.5 px-3 py-2 text-sm hover:bg-accent/40"
	href={reportHref(card.workspace, card.slug)}
>
	<span class="flex flex-wrap items-center gap-x-2 gap-y-0.5">
		<span class="font-mono text-xs text-muted-foreground">{fmtStamp(card.updated_at)}</span>
		<span class="font-medium">{card.title}</span>
		{#if card.status !== 'active'}<StatusBadge note={card} />{/if}
		{#if where}<span class="text-xs text-muted-foreground">{where}</span>{/if}
	</span>
	{#if parts}
		<span class="line-clamp-2 text-xs text-muted-foreground">
			{#each parts as part, i (i)}{#if i % 2}<mark
						class="rounded bg-warning/30 px-0.5 text-foreground">{part}</mark
					>{:else}{part}{/if}{/each}
		</span>
	{:else if text}
		<span class="line-clamp-2 text-xs text-muted-foreground">{text}</span>
	{/if}
</a>
