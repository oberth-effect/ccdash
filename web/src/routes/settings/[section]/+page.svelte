<script lang="ts">
	// Settings in sections: Devices (pairing, this device, push), Appearance
	// (theme, colours), Composer (the editor, vim, Edit in an editor, quick
	// replies, templates), Machines (hosts, a pointer to Claude state).
	import { page } from '$app/state';
	import Appearance from '#lib/components/settings/Appearance.svelte';
	import Composer from '#lib/components/settings/Composer.svelte';
	import Devices from '#lib/components/settings/Devices.svelte';
	import Machines from '#lib/components/settings/Machines.svelte';
	import { SECTIONS, type Section } from '#lib/settings.js';

	const views = {
		devices: Devices,
		appearance: Appearance,
		composer: Composer,
		machines: Machines
	};
	const section = $derived<Section>(
		SECTIONS.find(([id]) => id === page.params.section)?.[0] ?? 'devices'
	);
	const View = $derived(views[section]);
</script>

<div class="mx-auto flex max-w-3xl flex-col gap-6 p-3 sm:p-4">
	<nav class="-mx-3 overflow-x-auto px-3 sm:mx-0 sm:px-0" aria-label="Settings section">
		<div class="inline-flex w-fit gap-0.5 rounded-lg bg-muted p-[3px] text-sm whitespace-nowrap">
			{#each SECTIONS as [id, label] (id)}
				<a
					href="/settings/{id}"
					aria-current={id === section ? 'page' : undefined}
					class="flex items-center gap-1.5 rounded-md px-2.5 py-1 {id === section
						? 'bg-background font-medium text-foreground shadow-sm'
						: 'text-foreground/60 hover:text-foreground'}"
				>
					{label}
				</a>
			{/each}
		</div>
	</nav>
	<View />
</div>
