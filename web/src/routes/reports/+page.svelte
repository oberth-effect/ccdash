<script lang="ts">
	// Every report in the scope, newest first, with the one note search
	// narrowed to reports: a title, a summary or a word of the abstract
	// finds it. A report opens in its viewer.
	import { Input } from '#lib/components/ui/input/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import ReportRow from '#lib/components/reports/ReportRow.svelte';
	import { plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { dayKey, dayName } from '#lib/notes.js';
	import { NoteSearch, SORTS } from '#lib/notesearch.svelte.js';
	import { scope } from '#lib/scope.svelte.js';
	import type { Hit } from '#lib/api/types.js';

	let query = $state('');
	const search = new NoteSearch(() => ({
		text: query,
		workspace: scope.params().workspace,
		limit: 200
	}));
	// NOTE: the kind is what makes an empty query list everything
	search.kind = 'report';
	search.sort = 'updated';
	search.retired = false;

	type Day = { key: string; day: string; items: Hit[] };
	const days = $derived.by(() => {
		const out: Day[] = [];
		for (const h of search.shown ?? []) {
			const key = dayKey(h.updated_at);
			const last = out.at(-1);
			if (last && last.key === key) last.items.push(h);
			else out.push({ key, day: dayName(h.updated_at, live.now), items: [h] });
		}
		return out;
	});
	const total = $derived(search.shown?.length ?? 0);
</script>

<svelte:head>
	<title>Reports · ccdash</title>
</svelte:head>

<div class="mx-auto flex max-w-5xl flex-col gap-4 p-3 sm:p-4">
	<div class="flex flex-col gap-2">
		<Input
			type="search"
			bind:value={query}
			placeholder="Search reports…"
			aria-label="Search reports"
			class="max-w-md"
		/>
		<div class="flex flex-wrap items-center gap-1.5 text-xs">
			{#each SORTS as s (s.id)}
				<Button
					variant={search.sort === s.id ? 'secondary' : 'ghost'}
					size="xs"
					aria-pressed={search.sort === s.id}
					onclick={() => (search.sort = s.id)}>{s.label}</Button
				>
			{/each}
			<span class="mx-1 h-4 border-l" aria-hidden="true"></span>
			<label class="inline-flex items-center gap-1.5 text-muted-foreground">
				<input type="checkbox" bind:checked={search.retired} class="accent-primary" /> archived too
			</label>
			<span class="ml-auto text-muted-foreground">
				{search.searching ? 'Searching…' : plural(total, 'report')}
			</span>
		</div>
	</div>
	{#if !search.shown}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{:else if !total}
		<p class="text-sm text-muted-foreground">
			{#if query.trim()}
				Nothing matches.
			{:else}
				No reports yet. A session publishes one with the <code>ccdash:report</code> skill (<code
					>report_publish</code
				>): the hub keeps the file, this page lists it, and the viewer shows it with its abstract.
			{/if}
		</p>
	{:else}
		{#each days as d (d.key)}
			<section class="flex flex-col gap-1">
				<h2 class="text-sm font-semibold">
					{d.day} <span class="font-normal text-muted-foreground">{d.items.length}</span>
				</h2>
				<ul class="divide-y rounded-lg border bg-card">
					{#each d.items as hit (hit.workspace + hit.slug)}
						<li>
							<ReportRow
								card={hit}
								workspace={live.multiHost || !scope.params().workspace}
								snippet={query.trim() ? hit.snippet : undefined}
							/>
						</li>
					{/each}
				</ul>
			</section>
		{/each}
	{/if}
</div>
