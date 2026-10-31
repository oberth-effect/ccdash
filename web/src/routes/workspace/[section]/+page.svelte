<script lang="ts">
	// The Workspace area: the workspace root as one logical tree on every
	// machine, in sections (overview, units, machines, statistics,
	// configuration). The live view is polled here and handed to the
	// section shown.
	import { page } from '$app/state';
	import { Unauthorized } from '#lib/api/client.js';
	import { fetchWorkspace, type WorkspaceView } from '#lib/api/index.js';
	import Config from '#lib/components/workspace/Config.svelte';
	import Machines from '#lib/components/workspace/Machines.svelte';
	import Overview from '#lib/components/workspace/Overview.svelte';
	import Stats from '#lib/components/workspace/Stats.svelte';
	import UnitsSection from '#lib/components/workspace/UnitsSection.svelte';
	import { live } from '#lib/live/store.svelte.js';
	import { SECTIONS, type Section } from '#lib/workspace.js';

	const POLL_MS = 3000;

	const views = {
		overview: Overview,
		units: UnitsSection,
		machines: Machines,
		config: Config
	};

	const section = $derived<Section>(
		SECTIONS.find(([id]) => id === page.params.section)?.[0] ?? 'overview'
	);
	const View = $derived(section === 'stats' ? Overview : views[section]);

	let view = $state<WorkspaceView | null>(null);
	let error = $state('');
	let off = $state(false);

	async function load() {
		try {
			view = await fetchWorkspace();
			error = '';
			off = false;
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else if ((e as { status?: number }).status === 404) off = true;
			else error = (e as Error).message;
		}
	}

	$effect(() => {
		if (!live.ready) return;
		load();
		const timer = setInterval(load, POLL_MS);
		return () => clearInterval(timer);
	});

	let tabs = $state<HTMLElement | null>(null);
	/** Tabs hidden past the right edge: the row fades there. */
	let more = $state(false);
	const measure = () => {
		if (tabs) more = tabs.scrollLeft + tabs.clientWidth < tabs.scrollWidth - 2;
	};
	$effect(() => {
		void section;
		tabs
			?.querySelector('[aria-current="page"]')
			?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
		requestAnimationFrame(measure);
	});

	const needs = $derived(
		view
			? view.conflicts.length +
					view.held.length +
					view.new_dirs.filter((d) => d.state === 'waiting').length
			: 0
	);
</script>

<svelte:window onresize={measure} />

<div class="mx-auto flex max-w-6xl flex-col gap-4 p-3 sm:p-4">
	<nav
		class="-mx-3 overflow-x-auto px-3 sm:mx-0 sm:px-0 {more ? 'mask-r-from-85%' : ''}"
		aria-label="Workspace section"
		bind:this={tabs}
		onscroll={measure}
	>
		<div class="inline-flex w-fit gap-0.5 rounded-lg bg-muted p-[3px] text-sm whitespace-nowrap">
			{#each SECTIONS as [id, label] (id)}
				<a
					href="/workspace/{id}"
					aria-current={id === section ? 'page' : undefined}
					class="flex items-center gap-1.5 rounded-md px-2.5 py-1 {id === section
						? 'bg-background font-medium text-foreground shadow-sm'
						: 'text-foreground/60 hover:text-foreground'}"
				>
					{label}
					{#if id === 'overview' && needs}
						<span
							class="rounded-full bg-permission/15 px-1.5 text-[10px] leading-4 text-permission tabular-nums"
							>{needs}</span
						>
					{/if}
				</a>
			{/each}
		</div>
	</nav>
	{#if off}
		<p class="text-sm text-muted-foreground">
			The workspace sync is off on this hub (<code class="font-mono">workspace.enable</code>).
		</p>
	{:else if error && !view}
		<p class="text-sm text-critical" role="alert">{error}</p>
	{:else if view}
		{#if error}<p class="text-xs text-critical" role="alert">{error}</p>{/if}
		{#if section === 'stats'}
			<Stats {view} />
		{:else}
			<View {view} onchange={load} />
		{/if}
	{:else}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{/if}
</div>
