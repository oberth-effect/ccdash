<script lang="ts">
	// The Projects overview: per workspace in scope, the flagged notes and
	// a card per project; a search box over every note with kind, sort
	// and retired-notes toggles. Results can be picked across searches and
	// exported as an archive for someone without ccdash.
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import { Unauthorized } from '#lib/api/client.js';
	import { exportNotes, type Hit } from '#lib/api/index.js';
	import NoteRow from '#lib/components/notes/NoteRow.svelte';
	import SearchFilters from '#lib/components/notes/SearchFilters.svelte';
	import ProjectCard from '#lib/components/projects/ProjectCard.svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { newNoteHref, projectHref } from '#lib/notes.js';
	import { notePanel } from '#lib/notepanel.svelte.js';
	import { NoteSearch } from '#lib/notesearch.svelte.js';
	import { projects } from '#lib/projects.svelte.js';
	import { scope } from '#lib/scope.svelte.js';

	let query = $state('');
	const search = new NoteSearch(() => ({ text: query, workspace: scope.params().workspace }));

	/** The notes picked for an export, across searches. */
	let picked = $state<Record<string, Hit>>({});
	let exporting = $state(false);
	let exportError = $state('');
	const pickKey = (hit: Hit) => `${hit.workspace}/${hit.slug}`;
	const pickedCount = $derived(Object.keys(picked).length);
	const allPicked = $derived(search.shown?.every((hit) => picked[pickKey(hit)]) ?? false);

	function togglePick(hit: Hit) {
		const key = pickKey(hit);
		if (picked[key]) delete picked[key];
		else picked[key] = hit;
	}

	function pickAll() {
		const hits = search.shown ?? [];
		if (allPicked) for (const hit of hits) delete picked[pickKey(hit)];
		else for (const hit of hits) picked[pickKey(hit)] = hit;
	}

	async function runExport() {
		exporting = true;
		exportError = '';
		try {
			const blob = await exportNotes(
				Object.values(picked).map(({ workspace, slug }) => ({ workspace, slug }))
			);
			const link = document.createElement('a');
			link.href = URL.createObjectURL(blob);
			link.download = 'ccdash-notes.zip';
			link.click();
			URL.revokeObjectURL(link.href);
			picked = {};
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else exportError = (e as Error).message;
		} finally {
			exporting = false;
		}
	}

	$effect(() => {
		void live.notes.n;
		void projects.claimsSignature;
		const { workspace } = scope.params();
		if (live.ready) projects.schedule(workspace);
	});

	const workspaces = $derived((projects.data?.workspaces ?? []).filter((w) => w.total > 0));
</script>

<div class="mx-auto flex max-w-6xl flex-col gap-5 p-3 sm:p-4">
	<div class="flex flex-col gap-2">
		<Input
			type="search"
			bind:value={query}
			placeholder="Search notes…"
			aria-label="Search notes"
			class="max-w-md"
		/>
		<SearchFilters {search} />
	</div>
	{#if search.shown}
		<section class="flex flex-col gap-2">
			<div class="flex flex-wrap items-center gap-2">
				<h2 class="text-base font-semibold">
					{search.searching ? 'Searching…' : `Results (${search.shown.length})`}
				</h2>
				{#if search.shown.length}
					<Button variant="ghost" size="xs" onclick={pickAll}
						>{allPicked ? 'Unpick these' : 'Pick these'}</Button
					>
				{/if}
				{#if pickedCount}
					<Button size="xs" class="ml-auto" disabled={exporting} onclick={runExport}>
						<DownloadIcon /> Export {plural(pickedCount, 'note')}
					</Button>
					<Button variant="ghost" size="xs" onclick={() => (picked = {})}>Clear</Button>
				{/if}
			</div>
			{#if exportError}<p class="text-xs text-critical" role="alert">{exportError}</p>{/if}
			{#if search.shown.length}
				<ul class="divide-y rounded-lg border bg-card">
					{#each search.shown as hit (hit.workspace + hit.slug)}
						<li class="flex min-w-0 items-start">
							<input
								type="checkbox"
								class="mt-2.5 ml-3 accent-primary"
								aria-label="Pick {hit.title}"
								checked={Boolean(picked[pickKey(hit)])}
								onchange={() => togglePick(hit)}
							/>
							<div class="min-w-0 flex-1">
								<NoteRow
									card={hit}
									kind
									workspace
									project
									snippet={hit.snippet}
									onclick={() => notePanel.show(hit.workspace, hit.slug)}
								/>
							</div>
						</li>
					{/each}
				</ul>
			{:else if !search.searching}
				<p class="text-sm text-muted-foreground">Nothing matches.</p>
			{/if}
		</section>
	{:else if projects.error}
		<p class="text-sm text-critical" role="alert">{projects.error}</p>
	{:else if !projects.data}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{:else if !workspaces.length}
		<p class="text-sm text-muted-foreground">
			No notes yet. Sessions write them with the ccdash tools; <code>ccdash import memory</code> brings
			in memory files.
		</p>
	{:else}
		{#each workspaces as w (w.name)}
			<section class="flex flex-col gap-2">
				<div class="flex flex-wrap items-baseline gap-2">
					<a href={projectHref(w.name)} class="text-base font-semibold hover:underline">{w.name}</a>
					<span class="text-xs text-muted-foreground"
						>{plural(w.total, 'note')}{w.flagged.length
							? ` · ${w.flagged.length} flagged`
							: ''}</span
					>
					<DropdownMenu.Root>
						<DropdownMenu.Trigger>
							{#snippet child({ props })}
								<Button variant="ghost" size="xs" class="ml-auto" {...props}>
									<PlusIcon /> New <ChevronDownIcon class="size-3 opacity-60" />
								</Button>
							{/snippet}
						</DropdownMenu.Trigger>
						<DropdownMenu.Content align="end">
							<DropdownMenu.Item>
								{#snippet child({ props })}
									<a href={newNoteHref(w.name, 'issue')} {...props}>Issue</a>
								{/snippet}
							</DropdownMenu.Item>
							<DropdownMenu.Item>
								{#snippet child({ props })}
									<a href={newNoteHref(w.name, 'memory')} {...props}>Memory</a>
								{/snippet}
							</DropdownMenu.Item>
							<DropdownMenu.Item>
								{#snippet child({ props })}
									<a href={newNoteHref(w.name, 'doc')} {...props}>Document</a>
								{/snippet}
							</DropdownMenu.Item>
						</DropdownMenu.Content>
					</DropdownMenu.Root>
				</div>
				{#if w.flagged.length}
					<details class="rounded-lg border border-warning/50 bg-card">
						<summary class="cursor-pointer px-3 py-2 text-sm select-none">
							Flagged <span class="text-muted-foreground">{w.flagged.length}</span>
							<span class="text-xs text-muted-foreground">
								· stale memories: verify, rewrite or archive</span
							>
						</summary>
						<ul class="divide-y border-t">
							{#each w.flagged as c (c.slug)}
								<li>
									<NoteRow
										card={c}
										kind
										project
										reason
										onclick={() => notePanel.show(w.name, c.slug)}
									/>
								</li>
							{/each}
						</ul>
					</details>
				{/if}
				<div class="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
					{#each w.projects as p (p.project ?? '')}
						<ProjectCard ws={w.name} project={p} />
					{/each}
				</div>
			</section>
		{/each}
	{/if}
</div>
