<script lang="ts">
	// The graph tab, for exploring. The bar narrows what is shown: full-text
	// search (the search of Projects and Ctrl K), kinds, retired notes,
	// projects, isolated notes; Display sets how it looks and moves, in 2D or
	// 3D. A click selects a note: the inspector beside the graph shows what
	// it links to, opens a local graph around it or the shortest way to
	// another note. The slider plays the growth over time. Keys: `/` search,
	// `n` next match, `l` local graph, `p` path, `f` full screen, `d` 2D or
	// 3D, `0` fit, Esc steps back. `?note=<slug>` selects a note on arrival.
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import ExpandIcon from '@lucide/svelte/icons/expand';
	import MinusIcon from '@lucide/svelte/icons/minus';
	import PanelRightIcon from '@lucide/svelte/icons/panel-right';
	import PauseIcon from '@lucide/svelte/icons/pause';
	import PlayIcon from '@lucide/svelte/icons/play';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import ScanIcon from '@lucide/svelte/icons/scan';
	import ShrinkIcon from '@lucide/svelte/icons/shrink';
	import SlidersHorizontalIcon from '@lucide/svelte/icons/sliders-horizontal';
	import XIcon from '@lucide/svelte/icons/x';
	import type { Graph, GraphNode, LinkKind, NoteKind } from '#lib/api/types.js';
	import { Unauthorized } from '#lib/api/client.js';
	import { fetchGraph } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import { Toggle } from '#lib/components/ui/toggle/index.js';
	import { fmtDate, plural } from '#lib/format.js';
	import {
		components,
		degree,
		indexById,
		neighbourhood,
		pathEdges,
		projectColours,
		rankScale,
		shortestPath,
		shownGraph,
		slugIndex,
		type OrphanFilter,
		type Swatch
	} from '#lib/graph.js';
	import { live } from '#lib/live/store.svelte.js';
	import { KIND_COLOR, KIND_LABEL, KINDS } from '#lib/notes.js';
	import { notePanel } from '#lib/notepanel.svelte.js';
	import { NoteSearch } from '#lib/notesearch.svelte.js';
	import {
		DEFAULT_GRAPH,
		prefs,
		type GraphColour,
		type GraphLayout,
		type GraphSize
	} from '#lib/prefs.svelte.js';
	import GraphInspector from './GraphInspector.svelte';
	import NoteGraph from './NoteGraph.svelte';
	import NoteGraph3D from './NoteGraph3D.svelte';

	const PLAY_MS = 8000;
	const MATCHES = 200;
	const DEPTHS = [1, 2, 3, 4];
	const LINK_KINDS: [LinkKind, string][] = [
		['mentions', 'Mentions'],
		['relates', 'Relates'],
		['supersedes', 'Supersedes'],
		['blocks', 'Blocks']
	];
	const COLOURS: [GraphColour, string][] = [
		['kind', 'Kind'],
		['project', 'Project'],
		['age', 'Age'],
		['links', 'Links'],
		['recalls', 'Recalls']
	];
	const SIZES: [GraphSize, string][] = [
		['recalls', 'Recalls'],
		['links', 'Links'],
		['same', 'Same']
	];
	const LAYOUTS: [GraphLayout, string][] = [
		['force', 'Free'],
		['clusters', 'By project'],
		['timeline', 'Timeline']
	];
	const ORPHANS: [OrphanFilter, string][] = [
		['all', 'All notes'],
		['hide', 'Linked only'],
		['only', 'Unlinked only']
	];

	let { ws, project }: { ws: string; project: string | null } = $props();

	let graph = $state.raw<Graph | null>(null);
	let error = $state('');
	let retired = $state(false);
	let kinds = $state<Record<NoteKind, boolean>>({
		memory: true,
		issue: true,
		doc: true,
		progress: true
	});
	let links = $state<Record<LinkKind, boolean>>({
		mentions: true,
		relates: true,
		supersedes: true,
		blocks: true
	});
	let orphans = $state<OrphanFilter>('all');
	let pickedProject = $state<string | null>(null);
	let projectSet = $state(false);
	/** Upper bound on `created_at`; null = everything. */
	let until = $state<number | null>(null);
	let playing = $state(false);

	let query = $state('');
	let onlyMatches = $state(false);
	let matchAt = $state(-1);
	let selectedId = $state<number | null>(null);
	/** The local graph's note and how far it reaches. */
	let focusId = $state<number | null>(null);
	let depth = $state(2);
	/** Picking the other end of a path, from this note. */
	let pathFrom = $state<number | null>(null);
	/** The two ends of a found path. */
	let pathEnds = $state<[number, number] | null>(null);

	let full = $state(false);
	let nativeFull = false;
	let inspector = $state(true);
	/** Why 3D could not draw; 2D stands in. */
	let failed3d = $state('');
	let view2d = $state<NoteGraph | null>(null);
	let view3d = $state<NoteGraph3D | null>(null);
	let searchBox = $state<HTMLInputElement | null>(null);

	const search = new NoteSearch(() => ({ text: query, workspace: ws, limit: MATCHES }));
	$effect(() => {
		search.retired = retired;
	});

	const look = $derived(prefs.graph);
	const in3d = $derived(look.view === '3d' && !failed3d);
	const canvas = $derived(in3d ? view3d : view2d);

	async function load() {
		try {
			graph = await fetchGraph(ws, retired ? 'active,stale,superseded,archived' : undefined);
			error = '';
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		}
	}

	$effect(() => {
		void ws;
		void retired;
		void live.notes.n;
		untrack(load);
	});

	// The project page starts scoped to its project; the workspace page to all.
	$effect(() => {
		void project;
		untrack(() => {
			if (!projectSet) pickedProject = project;
		});
	});

	const all = $derived(graph?.nodes ?? []);
	const byId = $derived(indexById(all));
	const bySlug = $derived(slugIndex(all));
	const projectsSeen = $derived([...new Set(all.map((n) => n.project ?? ''))].sort());
	const span = $derived.by(() => {
		const times = all.map((n) => n.created_at);
		return times.length ? [Math.min(...times), Math.max(...times)] : [0, 0];
	});

	/** The notes the search found, as ids; null while there is no search. */
	const matches = $derived.by(() => {
		const hits = search.shown;
		if (!hits) return null;
		const ids = hits.map((h) => bySlug.get(h.slug)).filter((id) => id !== undefined);
		return new Set(ids);
	});
	const shown = $derived(
		shownGraph(
			{ nodes: all, edges: graph?.edges ?? [] },
			{
				kinds,
				project: pickedProject,
				until,
				links,
				orphans,
				focus: focusId,
				depth,
				matches,
				only: onlyMatches
			}
		)
	);
	const visibleIds = $derived(new Set(shown.nodes.map((n) => n.id)));
	/** The matches on show, in the hub's order, for `n`. */
	const matchList = $derived(
		(search.shown ?? [])
			.map((h) => bySlug.get(h.slug))
			.filter((id): id is number => id !== undefined && visibleIds.has(id))
	);
	const selected = $derived(selectedId !== null ? (byId.get(selectedId) ?? null) : null);
	const path = $derived(
		pathEnds ? (shortestPath(pathEnds[0], pathEnds[1], shown.adj) ?? []) : null
	);
	const pathLinks = $derived(path?.length ? pathEdges(path) : null);
	const lit = $derived.by(() => {
		if (path?.length) return new Set(path);
		if (selectedId !== null && visibleIds.has(selectedId)) {
			return neighbourhood(selectedId, 1, shown.adj);
		}
		return matches && !onlyMatches ? matches : null;
	});
	const orphanCount = $derived(shown.nodes.filter((n) => !degree(shown.adj, n.id)).length);
	const parts = $derived(
		components(
			shown.nodes.map((n) => n.id),
			shown.adj
		).length
	);

	const projectScale = $derived(projectColours(all));
	const scale = $derived.by((): { of: (n: GraphNode) => string; legend: Swatch[] } => {
		const ramp = (low: string, high: string, values: number[], value: (n: GraphNode) => number) => {
			const step = rankScale(values);
			return {
				of: (n: GraphNode) => step(value(n)),
				legend: [
					{ label: low, colour: 'var(--seq-1)' },
					{ label: high, colour: 'var(--seq-6)' }
				]
			};
		};
		switch (look.colour) {
			case 'project':
				return {
					of: (n) => projectScale.of.get(n.project ?? '') ?? 'var(--muted-foreground)',
					legend: projectScale.legend
				};
			case 'age':
				return ramp(
					'Oldest',
					'Newest',
					all.map((n) => n.created_at),
					(n) => n.created_at
				);
			case 'links':
				return ramp(
					'Fewest links',
					'Most links',
					shown.nodes.map((n) => degree(shown.adj, n.id)),
					(n) => degree(shown.adj, n.id)
				);
			case 'recalls':
				return ramp(
					'Least recalled',
					'Most recalled',
					all.map((n) => n.recalls),
					(n) => n.recalls
				);
			default:
				return { of: (n) => KIND_COLOR[n.kind], legend: [] };
		}
	});
	const radiusOf = $derived((n: GraphNode) => {
		if (look.size === 'same') return 5;
		if (look.size === 'links') return Math.min(16, 4 + Math.sqrt(degree(shown.adj, n.id)) * 2.2);
		return n.kind === 'progress' ? 3.5 : Math.min(14, 5 + Math.sqrt(n.recalls) * 2);
	});

	function select(node: GraphNode | null) {
		if (node && pathFrom !== null && pathFrom !== node.id) {
			pathEnds = [pathFrom, node.id];
			pathFrom = null;
		}
		selectedId = node?.id ?? null;
	}

	/** Selects a note from a list and brings it into view. */
	function walkTo(node: GraphNode) {
		select(node);
		canvas?.reveal(node.id);
	}

	const openNote = (node: GraphNode) => notePanel.show(ws, node.slug);

	function focusOn(node: GraphNode) {
		focusId = node.id;
		selectedId = node.id;
		pathEnds = null;
		pathFrom = null;
	}

	function startPath(node: GraphNode) {
		pathFrom = node.id;
		pathEnds = null;
	}

	function stepMatch(step: number) {
		if (!matchList.length) return;
		matchAt = (matchAt + step + matchList.length) % matchList.length;
		const node = byId.get(matchList[matchAt]);
		if (node) walkTo(node);
	}

	// A note named in the link, once it is there to select.
	let arrived = false;
	$effect(() => {
		const wanted = page.url.searchParams.get('note');
		if (!graph || arrived || !wanted) return;
		arrived = true;
		const id = bySlug.get(wanted);
		if (id === undefined) return;
		untrack(() => {
			selectedId = id;
			setTimeout(() => canvas?.reveal(id), 700);
		});
	});

	$effect(() => {
		void query;
		matchAt = -1;
	});

	async function enterFull() {
		full = true;
		try {
			// NOTE: the page, not the graph: the note panel and start sheet are portalled to `body`
			await document.documentElement.requestFullscreen();
			nativeFull = true;
		} catch {
			// a phone without the API: the graph still fills the window
			nativeFull = false;
		}
	}

	function leaveFull() {
		full = false;
		nativeFull = false;
		if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
	}

	$effect(() => () => {
		if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
	});

	function onfullscreenchange() {
		if (!document.fullscreenElement && nativeFull) {
			full = false;
			nativeFull = false;
		}
	}

	function stepBack(): boolean {
		if (pathFrom !== null) pathFrom = null;
		else if (pathEnds) pathEnds = null;
		else if (selectedId !== null) selectedId = null;
		else if (focusId !== null) focusId = null;
		else if (full) leaveFull();
		else return false;
		return true;
	}

	function onkeydown(event: KeyboardEvent) {
		if (event.ctrlKey || event.metaKey || event.altKey) return;
		const target = event.target as HTMLElement;
		if (target.closest('[role=dialog], [role=menu]')) return;
		if (target === searchBox) {
			if (event.key === 'Enter') {
				event.preventDefault();
				stepMatch(event.shiftKey ? -1 : 1);
			} else if (event.key === 'Escape') {
				query = '';
				searchBox?.blur();
			}
			return;
		}
		if (target.closest('input, textarea, select, [contenteditable]')) return;
		const handled = (() => {
			switch (event.key) {
				case '/':
					searchBox?.focus();
					return true;
				case 'n':
					stepMatch(1);
					return true;
				case 'N':
					stepMatch(-1);
					return true;
				case 'l':
					if (selected) focusOn(selected);
					return Boolean(selected);
				case 'p':
					if (selected) startPath(selected);
					return Boolean(selected);
				case 'f':
					if (full) leaveFull();
					else enterFull();
					return true;
				case 'd':
					prefs.setGraph({ view: look.view === '3d' ? '2d' : '3d' });
					return true;
				case '0':
					canvas?.fit();
					return true;
				case 'Enter':
					if (selected) openNote(selected);
					return Boolean(selected);
				case 'Escape':
					return stepBack();
			}
			return false;
		})();
		if (handled) event.preventDefault();
	}

	let frame = 0;
	function play() {
		if (playing) {
			playing = false;
			cancelAnimationFrame(frame);
			return;
		}
		playing = true;
		const [from, to] = span;
		const start = performance.now();
		const step = (now: number) => {
			const t = Math.min(1, (now - start) / PLAY_MS);
			until = from + (to - from) * t;
			if (t < 1 && playing) frame = requestAnimationFrame(step);
			else {
				playing = false;
				until = null;
			}
		};
		frame = requestAnimationFrame(step);
	}

	const setPhysics = (changes: Partial<typeof look.physics>) =>
		prefs.setGraph({ physics: { ...look.physics, ...changes } });
</script>

<svelte:window {onkeydown} />
<svelte:document {onfullscreenchange} />

{#snippet choice<T extends string>(
	name: string,
	value: T,
	options: [T, string][],
	set: (v: T) => void
)}
	<div class="flex flex-col gap-1">
		<span class="text-xs text-muted-foreground">{name}</span>
		<div class="flex flex-wrap gap-1">
			{#each options as [id, label] (id)}
				<Button
					size="xs"
					variant={value === id ? 'secondary' : 'ghost'}
					aria-pressed={value === id}
					onclick={() => set(id)}>{label}</Button
				>
			{/each}
		</div>
	</div>
{/snippet}

{#snippet slider(
	name: string,
	value: number,
	min: number,
	max: number,
	step: number,
	set: (v: number) => void
)}
	<label class="flex flex-col gap-1 text-xs text-muted-foreground">
		<span class="flex justify-between"
			>{name}<span class="text-foreground tabular-nums">{value}</span></span
		>
		<input
			type="range"
			class="accent-primary"
			{min}
			{max}
			{step}
			{value}
			oninput={(e) => set(Number((e.currentTarget as HTMLInputElement).value))}
		/>
	</label>
{/snippet}

<div
	class={full ? 'fixed inset-0 z-40 flex flex-col gap-2 bg-background p-2' : 'flex flex-col gap-3'}
>
	<div class="flex flex-wrap items-center gap-2 text-xs">
		<Input
			bind:ref={searchBox}
			type="search"
			bind:value={query}
			placeholder="Search notes… ( / )"
			aria-label="Search the graph"
			class="h-8 w-52 text-xs"
		/>
		{#if matches}
			<span class="text-muted-foreground tabular-nums"
				>{search.searching
					? 'Searching…'
					: `${matchAt >= 0 ? `${matchAt + 1} of ` : ''}${plural(matchList.length, 'match', 'matches')}`}</span
			>
			{#if matchList.length > 1}
				<Button variant="ghost" size="xs" title="Next match (n)" onclick={() => stepMatch(1)}
					>Next</Button
				>
			{/if}
			<Toggle size="sm" variant="outline" bind:pressed={onlyMatches}>Only matches</Toggle>
		{/if}
		{#each KINDS as kind (kind)}
			<Toggle
				size="sm"
				variant="outline"
				pressed={kinds[kind]}
				onPressedChange={(on) => (kinds = { ...kinds, [kind]: on })}
				aria-label={KIND_LABEL[kind]}
			>
				<i
					class="inline-block size-2 rounded-full"
					style="background: {KIND_COLOR[kind]}"
					aria-hidden="true"
				></i>
				{KIND_LABEL[kind]}
			</Toggle>
		{/each}
		<Toggle size="sm" variant="outline" bind:pressed={retired}>Retired notes</Toggle>
		{#if projectsSeen.length > 1}
			<select
				class="h-8 rounded-lg border border-input bg-transparent px-2 text-xs dark:bg-input/30"
				value={pickedProject ?? '*'}
				onchange={(e) => {
					const v = (e.currentTarget as HTMLSelectElement).value;
					pickedProject = v === '*' ? null : v;
					projectSet = true;
				}}
				aria-label="Project"
			>
				<option value="*">All projects</option>
				{#each projectsSeen as p (p)}<option value={p}>{p || 'Workspace-wide'}</option>{/each}
			</select>
		{/if}
		<select
			class="h-8 rounded-lg border border-input bg-transparent px-2 text-xs dark:bg-input/30"
			bind:value={orphans}
			aria-label="Notes without a link"
		>
			{#each ORPHANS as [id, label] (id)}<option value={id}>{label}</option>{/each}
		</select>
		<span class="ml-auto flex items-center gap-1.5">
			<span class="text-muted-foreground tabular-nums"
				>{plural(shown.nodes.length, 'note')} · {plural(shown.edges.length, 'link')}</span
			>
			<Popover.Root>
				<Popover.Trigger>
					{#snippet child({ props })}
						<Button variant="outline" size="sm" {...props}>
							<SlidersHorizontalIcon /> Display
						</Button>
					{/snippet}
				</Popover.Trigger>
				<Popover.Content align="end" class="flex w-80 flex-col gap-3 p-3">
					{@render choice('Colour', look.colour, COLOURS, (colour) => prefs.setGraph({ colour }))}
					{@render choice('Size', look.size, SIZES, (size) => prefs.setGraph({ size }))}
					{#if !in3d}
						{@render choice('Layout', look.layout, LAYOUTS, (layout) => prefs.setGraph({ layout }))}
					{/if}
					<div class="flex flex-col gap-1">
						<span class="text-xs text-muted-foreground">Links</span>
						<div class="flex flex-wrap gap-1">
							{#each LINK_KINDS as [kind, label] (kind)}
								<Toggle
									size="sm"
									variant="outline"
									pressed={links[kind]}
									onPressedChange={(on) => (links = { ...links, [kind]: on })}>{label}</Toggle
								>
							{/each}
						</div>
					</div>
					{@render slider('Repulsion', look.physics.repulsion, 5, 200, 5, (repulsion) =>
						setPhysics({ repulsion })
					)}
					{@render slider('Link length', look.physics.distance, 5, 120, 5, (distance) =>
						setPhysics({ distance })
					)}
					{@render slider('Gravity', look.physics.gravity, 0, 0.3, 0.01, (gravity) =>
						setPhysics({ gravity })
					)}
					<Button
						variant="ghost"
						size="xs"
						class="self-start"
						onclick={() => prefs.setGraph({ ...DEFAULT_GRAPH, view: look.view })}>Reset</Button
					>
				</Popover.Content>
			</Popover.Root>
			<span class="flex overflow-hidden rounded-lg border">
				{#each ['2d', '3d'] as const as v (v)}
					<button
						type="button"
						class="px-2 py-1.5 text-xs uppercase {look.view === v
							? 'bg-secondary text-secondary-foreground'
							: 'text-muted-foreground hover:bg-accent/40'}"
						aria-pressed={look.view === v}
						title="Switch view (d)"
						onclick={() => {
							failed3d = '';
							prefs.setGraph({ view: v });
						}}>{v}</button
					>
				{/each}
			</span>
			<Button
				variant="outline"
				size="icon-sm"
				aria-label={inspector ? 'Hide the side panel' : 'Show the side panel'}
				aria-pressed={inspector}
				title="Side panel"
				onclick={() => (inspector = !inspector)}
			>
				<PanelRightIcon />
			</Button>
			<Button
				variant="outline"
				size="icon-sm"
				aria-label={full ? 'Leave full screen' : 'Full screen'}
				title="Full screen (f)"
				onclick={() => (full ? leaveFull() : enterFull())}
			>
				{#if full}<ShrinkIcon />{:else}<ExpandIcon />{/if}
			</Button>
		</span>
	</div>
	{#if failed3d}
		<p class="text-xs text-warning" role="status">3D cannot draw here: {failed3d}. Showing 2D.</p>
	{/if}
	{#if error}
		<p class="text-sm text-critical" role="alert">{error}</p>
	{:else if !graph}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{:else}
		<div
			class="grid min-h-0 flex-1 gap-2 {inspector ? 'lg:grid-cols-[minmax(0,1fr)_19rem]' : ''} {full
				? 'grid-rows-[minmax(0,1fr)]'
				: ''}"
		>
			<div
				class="relative overflow-hidden rounded-lg border bg-card {full
					? 'min-h-0'
					: 'h-[68vh] min-h-72'}"
			>
				{#if in3d}
					<NoteGraph3D
						bind:this={view3d}
						nodes={shown.nodes}
						edges={shown.edges}
						adj={shown.adj}
						colour={scale.of}
						radius={radiusOf}
						{lit}
						path={pathLinks}
						selected={selectedId}
						physics={look.physics}
						onselect={select}
						onopen={openNote}
						onfail={(why) => (failed3d = why)}
					/>
				{:else}
					<NoteGraph
						bind:this={view2d}
						nodes={shown.nodes}
						edges={shown.edges}
						adj={shown.adj}
						colour={scale.of}
						radius={radiusOf}
						{lit}
						path={pathLinks}
						selected={selectedId}
						layout={look.layout}
						physics={look.physics}
						onselect={select}
						onopen={openNote}
					/>
				{/if}
				<div class="absolute top-2 left-2 flex max-w-[70%] flex-col items-start gap-1 text-xs">
					{#if focusId !== null}
						<div class="flex items-center gap-1.5 rounded-md border bg-popover px-2 py-1 shadow-sm">
							<span class="max-w-52 truncate"
								>Around <b class="font-medium">{byId.get(focusId)?.title ?? 'a note'}</b></span
							>
							<span class="flex" role="group" aria-label="Links away">
								{#each DEPTHS as d (d)}
									<Button
										size="xs"
										variant={depth === d ? 'secondary' : 'ghost'}
										class="h-5 min-w-5 px-1"
										aria-pressed={depth === d}
										title={plural(d, 'link') + ' away'}
										onclick={() => (depth = d)}>{d}</Button
									>
								{/each}
							</span>
							<Button
								variant="ghost"
								size="icon-xs"
								aria-label="Show every note"
								onclick={() => (focusId = null)}><XIcon /></Button
							>
						</div>
					{/if}
					{#if pathFrom !== null}
						<div class="flex items-center gap-1.5 rounded-md border bg-popover px-2 py-1 shadow-sm">
							<span class="max-w-60 truncate"
								>Pick a note: the way from <b class="font-medium">{byId.get(pathFrom)?.title}</b
								></span
							>
							<Button
								variant="ghost"
								size="icon-xs"
								aria-label="Cancel"
								onclick={() => (pathFrom = null)}><XIcon /></Button
							>
						</div>
					{/if}
					{#if pathEnds}
						<Button variant="outline" size="xs" class="bg-popover" onclick={() => (pathEnds = null)}
							><XIcon /> Clear the path</Button
						>
					{/if}
				</div>
				<div class="absolute top-2 right-2 flex flex-col gap-1">
					<Button
						variant="outline"
						size="icon-sm"
						aria-label="Zoom in"
						onclick={() => canvas?.zoomBy(1.3)}
					>
						<PlusIcon />
					</Button>
					<Button
						variant="outline"
						size="icon-sm"
						aria-label="Zoom out"
						onclick={() => canvas?.zoomBy(1 / 1.3)}
					>
						<MinusIcon />
					</Button>
					<Button
						variant="outline"
						size="icon-sm"
						aria-label="Fit"
						title="Fit (0)"
						onclick={() => canvas?.fit()}
					>
						<ScanIcon />
					</Button>
				</div>
				{#if look.layout === 'timeline' && !in3d}
					<p
						class="pointer-events-none absolute right-2 bottom-1.5 left-2 flex justify-between text-xs text-muted-foreground"
					>
						<span>← oldest</span><span>newest →</span>
					</p>
				{/if}
				{#if !shown.nodes.length}
					<p class="absolute inset-0 grid place-items-center text-sm text-muted-foreground">
						Nothing to show with these filters.
					</p>
				{/if}
			</div>
			{#if inspector}
				<GraphInspector
					{ws}
					nodes={shown.nodes}
					{byId}
					adj={shown.adj}
					{parts}
					orphans={orphanCount}
					{selected}
					{path}
					pathFrom={pathFrom !== null ? (byId.get(pathFrom) ?? null) : null}
					onselect={walkTo}
					onopen={openNote}
					onfocus={focusOn}
					onpath={startPath}
					class={full ? 'max-h-full' : 'max-h-[68vh]'}
				/>
			{/if}
		</div>
		{#if span[1] > span[0]}
			<div class="flex items-center gap-2 text-xs">
				<Button
					variant="outline"
					size="icon-sm"
					aria-label={playing ? 'Pause' : 'Play the growth'}
					onclick={play}
				>
					{#if playing}<PauseIcon />{:else}<PlayIcon />{/if}
				</Button>
				<span class="w-20 shrink-0 text-muted-foreground tabular-nums">{fmtDate(span[0])}</span>
				<input
					type="range"
					class="flex-1 accent-foreground"
					min={span[0]}
					max={span[1]}
					step={Math.max(1, Math.round((span[1] - span[0]) / 500))}
					value={until ?? span[1]}
					oninput={(e) => {
						playing = false;
						const v = Number((e.currentTarget as HTMLInputElement).value);
						until = v >= span[1] ? null : v;
					}}
					aria-label="Created before"
				/>
				<span class="w-20 shrink-0 text-right text-muted-foreground tabular-nums"
					>{fmtDate(until ?? span[1])}</span
				>
			</div>
		{/if}
		<p class="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
			{#each scale.legend as s (s.label)}
				<span class="inline-flex items-center gap-1.5"
					><i
						class="inline-block size-2 rounded-full"
						style="background: {s.colour}"
						aria-hidden="true"
					></i>{s.label}</span
				>
			{/each}
			<span>Size: {look.size === 'same' ? 'the same' : look.size}</span>
			<span
				><span class="inline-block w-5 border-t border-muted-foreground align-middle"></span> mentions</span
			>
			<span
				><span class="inline-block w-5 border-t border-dotted border-muted-foreground align-middle"
				></span> relates (progress on an issue)</span
			>
			<span
				><span
					class="inline-block w-5 border-t-2 border-dashed border-muted-foreground align-middle"
				></span> supersedes</span
			>
			<span
				><span class="inline-block w-5 border-t border-critical align-middle"></span> blocks</span
			>
			<span>Dashed ring: stale · faded: retired</span>
		</p>
	{/if}
</div>
