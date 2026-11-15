<script lang="ts">
	// The panel's help: every key in one place, by where it works. `?`
	// outside a field opens it; so do the sidebar and the composer's menu.
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { help, KEY_GROUPS } from '#lib/help.svelte.js';

	function onkeydown(event: KeyboardEvent) {
		if (event.key !== '?' || event.ctrlKey || event.metaKey || event.altKey) return;
		const target = event.target as HTMLElement;
		if (target.closest('input, textarea, select, [contenteditable], [role=dialog]')) return;
		event.preventDefault();
		help.show();
	}
</script>

<svelte:window {onkeydown} />

<Dialog.Root bind:open={help.open}>
	<Dialog.Content
		class="flex max-h-[min(88dvh,48rem)] w-[min(96vw,56rem)] max-w-none flex-col gap-3 sm:max-w-none"
	>
		<Dialog.Header>
			<Dialog.Title>Keys and help</Dialog.Title>
			<Dialog.Description>
				What the keyboard does in the panel. Ctrl is ⌘ on a Mac. More in
				<a
					class="underline"
					href="https://tsandrini.github.io/ccdash/panel.html"
					target="_blank"
					rel="noopener noreferrer">the panel guide</a
				>.
			</Dialog.Description>
		</Dialog.Header>
		<div class="-mr-2 grid min-h-0 gap-x-8 gap-y-4 overflow-y-auto pr-2 md:grid-cols-2">
			{#each KEY_GROUPS as group (group.title)}
				<section class="flex flex-col gap-1.5">
					<h3 class="text-sm font-semibold">{group.title}</h3>
					{#if group.note}<p class="text-xs text-muted-foreground">{group.note}</p>{/if}
					<dl class="grid grid-cols-[auto_1fr] items-baseline gap-x-3 gap-y-1 text-sm">
						{#each group.keys as [keys, what] (keys)}
							<dt>
								<kbd
									class="rounded border bg-muted px-1.5 py-0.5 font-mono text-xs whitespace-nowrap text-muted-foreground"
									>{keys}</kbd
								>
							</dt>
							<dd>{what}</dd>
						{/each}
					</dl>
				</section>
			{/each}
		</div>
	</Dialog.Content>
</Dialog.Root>
