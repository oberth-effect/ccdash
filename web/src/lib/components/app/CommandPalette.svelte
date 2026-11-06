<script lang="ts">
	// Ctrl K from anywhere: the sessions and pages matching the text (and a
	// search of the past sessions for it), and the
	// note search the Projects page has (`NoteSearch`). The hub ranks the
	// notes, so nothing here filters them again: a hit can match in its body
	// alone. A note opens beside the page; from the composer's `[[` it is
	// handed back instead, and only notes are offered.
	import { goto } from '$app/navigation';
	import ArchiveRestoreIcon from '@lucide/svelte/icons/archive-restore';
	import * as Command from '#lib/components/ui/command/index.js';
	import type { Hit } from '#lib/api/index.js';
	import NoteRow from '#lib/components/notes/NoteRow.svelte';
	import SearchFilters from '#lib/components/notes/SearchFilters.svelte';
	import { live } from '#lib/live/store.svelte.js';
	import { areas } from '#lib/nav.js';
	import { notePanel } from '#lib/notepanel.svelte.js';
	import { reportHref } from '#lib/notes.js';
	import { NoteSearch } from '#lib/notesearch.svelte.js';
	import { palette } from '#lib/palette.svelte.js';
	import { PAST, pastHref } from '#lib/past.js';
	import { scope } from '#lib/scope.svelte.js';
	import { ACTIVITY, label, shown } from '#lib/session.js';

	const search = new NoteSearch(() => ({
		text: palette.query,
		workspace: scope.params().workspace,
		enabled: palette.open
	}));

	// NOTE: a filter left on would hide the next search's hits with no sign why
	$effect(() => {
		if (!palette.open) search.kind = '';
	});

	const words = $derived(palette.query.toLowerCase().split(/\s+/).filter(Boolean));
	const matches = (text: string) => {
		const haystack = text.toLowerCase();
		return words.every((word) => haystack.includes(word));
	};

	const sessions = $derived(
		live.list
			.filter(
				(s) =>
					s.activity !== 'gone' &&
					matches(`${label(s)} ${s.id} ${s.workspace ?? ''} ${s.project ?? ''} ${s.host}`)
			)
			.sort((a, b) => b.last_event_at - a.last_event_at)
	);
	const pages = $derived([
		...areas.filter((area) => matches(area.label)),
		...(matches('past sessions resume old closed history')
			? [{ href: PAST, label: 'Past sessions', icon: ArchiveRestoreIcon }]
			: [])
	]);
	const pastQuery = $derived(palette.query.trim());
	const notes = $derived(search.shown ?? []);
	const empty = $derived(
		!notes.length && (palette.notesOnly || (!sessions.length && !pages.length && !pastQuery))
	);

	function pickNote(hit: Hit) {
		const onNote = palette.onNote;
		palette.close();
		if (onNote) onNote(hit);
		else if (hit.kind === 'report') goto(reportHref(hit.workspace, hit.slug));
		else notePanel.show(hit.workspace, hit.slug);
	}

	function go(href: string) {
		palette.close();
		goto(href);
	}

	function onkeydown(event: KeyboardEvent) {
		if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
			event.preventDefault();
			if (palette.open) palette.close();
			else palette.show();
		}
	}
</script>

<svelte:window {onkeydown} />

<Command.Dialog
	bind:open={palette.open}
	onOpenChange={(open) => {
		if (!open) palette.close();
	}}
	title={palette.notesOnly ? 'Insert a note link' : 'Search'}
	description="Sessions, notes and pages"
	shouldFilter={false}
	class="top-[10vh] sm:max-w-3xl"
>
	<Command.Input
		placeholder={palette.notesOnly ? 'Search notes…' : 'Sessions, notes, pages…'}
		bind:value={palette.query}
	/>
	<SearchFilters {search} class="border-b px-2 py-1.5" />
	<Command.List class="max-h-[70vh]">
		{#if empty}
			<p class="py-6 text-center text-sm text-muted-foreground">
				{search.searching
					? 'Searching…'
					: words.length || search.kind
						? 'Nothing found.'
						: 'Type to search every note.'}
			</p>
		{/if}
		{#if !palette.notesOnly && sessions.length}
			<Command.Group heading="Sessions">
				{#each sessions as s (s.id)}
					<Command.Item value="session {s.id}" onSelect={() => go(`/sessions/${s.id}`)}>
						<span class="truncate">{label(s)}</span>
						<span class="ml-auto text-xs text-muted-foreground">{ACTIVITY[shown(s)].label}</span>
					</Command.Item>
				{/each}
			</Command.Group>
		{/if}
		{#if !palette.notesOnly && pastQuery}
			<Command.Group heading="Past sessions">
				<Command.Item
					value="past {pastQuery}"
					onSelect={() =>
						go(pastHref({ q: pastQuery, hosts: [], project: '', when: '', sort: '' }))}
				>
					<ArchiveRestoreIcon class="size-4 opacity-70" />
					<span class="truncate">Search past sessions for “{pastQuery}”</span>
				</Command.Item>
			</Command.Group>
		{/if}
		{#if notes.length}
			<Command.Group heading="Notes ({notes.length})">
				{#each notes as hit (hit.workspace + hit.slug)}
					<Command.Item value="note {hit.workspace} {hit.slug}" onSelect={() => pickNote(hit)}>
						<NoteRow card={hit} kind workspace project snippet={hit.snippet} />
					</Command.Item>
				{/each}
			</Command.Group>
		{/if}
		{#if !palette.notesOnly && pages.length}
			<Command.Group heading="Pages">
				{#each pages as area (area.href)}
					<Command.Item value="page {area.label}" onSelect={() => go(area.href)}>
						<area.icon class="size-4 opacity-70" />
						{area.label}
					</Command.Item>
				{/each}
			</Command.Group>
		{/if}
	</Command.List>
</Command.Dialog>
