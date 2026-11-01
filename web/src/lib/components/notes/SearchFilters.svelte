<script lang="ts">
	// What narrows and orders a note search: kind, sort, retired notes.
	import { Button } from '#lib/components/ui/button/index.js';
	import { KIND_LABEL, KINDS } from '#lib/notes.js';
	import { SORTS, type NoteSearch } from '#lib/notesearch.svelte.js';
	import { cn } from '#lib/utils.js';

	let { search, class: className }: { search: NoteSearch; class?: string } = $props();
</script>

<div class={cn('flex flex-wrap items-center gap-1.5 text-xs', className)}>
	<Button
		variant={search.kind === '' ? 'secondary' : 'ghost'}
		size="xs"
		aria-pressed={search.kind === ''}
		onclick={() => (search.kind = '')}>All kinds</Button
	>
	{#each KINDS as k (k)}
		<Button
			variant={search.kind === k ? 'secondary' : 'ghost'}
			size="xs"
			aria-pressed={search.kind === k}
			onclick={() => (search.kind = search.kind === k ? '' : k)}>{KIND_LABEL[k]}</Button
		>
	{/each}
	<span class="mx-1 h-4 border-l" aria-hidden="true"></span>
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
		<input type="checkbox" bind:checked={search.retired} class="accent-primary" /> retired too
	</label>
</div>
