<script lang="ts">
	// Beside the graph: with nothing selected, the numbers of what is shown
	// and the best connected notes; with a note, what it is and what it links
	// to, to walk the graph by; with a found path, its steps. Selecting here
	// is the same as clicking in the graph.
	import type { GraphNode } from '#lib/api/types.js';
	import KindBadge from '#lib/components/notes/KindBadge.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { fmtDate, plural } from '#lib/format.js';
	import { degree, type Adjacency } from '#lib/graph.js';
	import { cn } from '#lib/utils.js';
	import WorkOnIssue from './WorkOnIssue.svelte';

	const TOP = 8;
	const LINKED = 40;

	let {
		ws,
		nodes,
		byId,
		adj,
		parts,
		orphans,
		selected,
		path,
		pathFrom,
		onselect,
		onopen,
		onfocus,
		onpath,
		class: className
	}: {
		ws: string;
		/** What the graph shows now. */
		nodes: GraphNode[];
		byId: Map<number, GraphNode>;
		adj: Adjacency;
		parts: number;
		orphans: number;
		selected: GraphNode | null;
		/** A found path as note ids; empty when there is none. */
		path: number[] | null;
		pathFrom: GraphNode | null;
		onselect: (node: GraphNode) => void;
		onopen: (node: GraphNode) => void;
		onfocus: (node: GraphNode) => void;
		onpath: (node: GraphNode) => void;
		class?: string;
	} = $props();

	const links = $derived(nodes.reduce((sum, n) => sum + degree(adj, n.id), 0) / 2);
	const hubs = $derived(
		nodes
			.filter((n) => degree(adj, n.id) > 0)
			.sort((a, b) => degree(adj, b.id) - degree(adj, a.id))
			.slice(0, TOP)
	);
	const neighbours = $derived(
		selected
			? [...(adj.get(selected.id) ?? [])]
					.map((id) => byId.get(id))
					.filter((n): n is GraphNode => Boolean(n))
					.sort((a, b) => degree(adj, b.id) - degree(adj, a.id))
			: []
	);
	const open = $derived(selected?.kind === 'issue' && selected.status !== 'archived');
</script>

{#snippet row(n: GraphNode, count?: number)}
	<button
		type="button"
		class="flex w-full min-w-0 items-center gap-2 rounded px-1.5 py-1 text-left text-xs hover:bg-accent/40"
		onclick={() => onselect(n)}
	>
		<KindBadge kind={n.kind} />
		<span class="min-w-0 flex-1 truncate">{n.title}</span>
		{#if count !== undefined}<span class="shrink-0 text-muted-foreground tabular-nums">{count}</span
			>{/if}
	</button>
{/snippet}

<aside
	class={cn(
		'flex min-h-0 flex-col gap-3 overflow-y-auto rounded-lg border bg-card p-3 text-sm',
		className
	)}
>
	{#if path}
		<section class="flex flex-col gap-1">
			<h3 class="text-xs font-semibold text-muted-foreground uppercase">
				{path.length ? plural(path.length - 1, 'link') : 'No way between them'}
			</h3>
			{#if !path.length}
				<p class="text-xs text-muted-foreground">
					These two are in separate parts of what is shown. Showing more kinds or retired notes may
					connect them.
				</p>
			{/if}
			<ol class="flex flex-col">
				{#each path as id, i (id)}
					{@const n = byId.get(id)}
					{#if n}
						<li class="flex items-center gap-1">
							<span class="w-4 shrink-0 text-right text-xs text-muted-foreground tabular-nums"
								>{i + 1}</span
							>
							<span class="min-w-0 flex-1">{@render row(n)}</span>
						</li>
					{/if}
				{/each}
			</ol>
		</section>
	{/if}
	{#if selected}
		<section class="flex flex-col gap-2">
			<div class="flex items-center gap-2 text-xs">
				<KindBadge kind={selected.kind} />
				{#if selected.status !== 'active'}<span class="text-muted-foreground"
						>{selected.status}</span
					>{/if}
				{#if selected.project}<span class="truncate text-muted-foreground">{selected.project}</span
					>{/if}
			</div>
			<button
				type="button"
				class="text-left font-medium break-words hover:underline"
				onclick={() => onopen(selected)}>{selected.title}</button
			>
			<p class="text-xs text-muted-foreground">
				{plural(neighbours.length, 'link')} · {plural(selected.recalls, 'recall')} · created
				{fmtDate(selected.created_at)}, updated {fmtDate(selected.updated_at)}
			</p>
			<div class="flex flex-wrap gap-1.5">
				<Button size="xs" onclick={() => onopen(selected)}>Open</Button>
				<Button
					size="xs"
					variant="outline"
					title="Only this note and what is around it (l)"
					onclick={() => onfocus(selected)}>Local graph</Button
				>
				<Button
					size="xs"
					variant="outline"
					title="Then pick another note: the shortest way between them (p)"
					disabled={pathFrom?.id === selected.id}
					onclick={() => onpath(selected)}>Path from here</Button
				>
				{#if open}<WorkOnIssue {ws} issue={selected} />{/if}
			</div>
		</section>
		{#if neighbours.length}
			<section class="flex flex-col gap-1">
				<h3 class="text-xs font-semibold text-muted-foreground uppercase">Linked notes</h3>
				{#each neighbours.slice(0, LINKED) as n (n.id)}
					{@render row(n, degree(adj, n.id))}
				{/each}
				{#if neighbours.length > LINKED}
					<p class="px-1.5 text-xs text-muted-foreground">
						and {neighbours.length - LINKED} more
					</p>
				{/if}
			</section>
		{/if}
	{:else if !path}
		<section class="flex flex-col gap-1">
			<h3 class="text-xs font-semibold text-muted-foreground uppercase">What is shown</h3>
			<dl class="grid grid-cols-[1fr_auto] gap-x-3 text-xs">
				<dt class="text-muted-foreground">Notes</dt>
				<dd class="tabular-nums">{nodes.length}</dd>
				<dt class="text-muted-foreground">Links</dt>
				<dd class="tabular-nums">{links}</dd>
				<dt class="text-muted-foreground">Without a link</dt>
				<dd class="tabular-nums">{orphans}</dd>
				<dt class="text-muted-foreground">Separate parts</dt>
				<dd class="tabular-nums">{parts}</dd>
			</dl>
		</section>
		{#if hubs.length}
			<section class="flex flex-col gap-1">
				<h3 class="text-xs font-semibold text-muted-foreground uppercase">Most linked</h3>
				{#each hubs as n (n.id)}{@render row(n, degree(adj, n.id))}{/each}
			</section>
		{/if}
		<p class="text-xs text-muted-foreground">
			Click a note to see what it links to, double click to read it.
		</p>
	{/if}
</aside>
