<script lang="ts">
	// Settings › Appearance: the theme (system, light, dark) and the colour
	// scheme, including the one a machine's wallpaper gives.
	import { Button } from '#lib/components/ui/button/index.js';
	import { live } from '#lib/live/store.svelte.js';
	import { prefs, type Theme } from '#lib/prefs.svelte.js';
	import { paletteSwatch, SCHEMES, swatch, WALLPAPER } from '#lib/theme.js';

	const reporting = $derived(live.hosts.filter((h) => h.palette));
	const wallpaper = $derived(
		[prefs.followed?.host, prefs.followed?.palette.wallpaper].filter(Boolean).join(' · ')
	);
	const modes = (s: (typeof SCHEMES)[number]) =>
		s.css || (s.dark && s.light) ? 'Light and dark' : s.dark ? 'Dark only' : 'Light only';
</script>

<section class="flex flex-col gap-2">
	<h2 class="text-base font-semibold">Theme</h2>
	<div class="flex gap-2">
		{#each ['auto', 'light', 'dark'] as t (t)}
			<Button
				variant={prefs.theme === t ? 'default' : 'outline'}
				size="sm"
				onclick={() => prefs.setTheme(t as Theme)}>{t}</Button
			>
		{/each}
	</div>
</section>

<section class="flex flex-col gap-2">
	<h2 class="text-base font-semibold">Colours</h2>
	{#snippet choice(id: string, name: string, sub: string, colours: string[])}
		<button
			type="button"
			class="flex min-w-0 flex-col gap-1.5 rounded-lg border bg-card p-2 text-left transition-colors hover:bg-accent/40 {prefs.scheme ===
			id
				? 'border-primary ring-1 ring-primary'
				: ''}"
			aria-pressed={prefs.scheme === id}
			onclick={() => prefs.setScheme(id)}
		>
			<span class="flex h-6 overflow-hidden rounded border" aria-hidden="true">
				{#each colours as colour, i (i)}<i class="flex-1" style:background={colour}></i>{/each}
			</span>
			<span class="truncate text-sm font-medium">{name}</span>
			<span class="truncate text-xs text-muted-foreground">{sub}</span>
		</button>
	{/snippet}
	<div class="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
		{@render choice(
			WALLPAPER,
			'Wallpaper',
			wallpaper || 'No machine has reported one',
			prefs.followed ? paletteSwatch(prefs.followed.palette) : []
		)}
		{#each SCHEMES as s (s.id)}
			{@render choice(s.id, s.label, modes(s), swatch(s))}
		{/each}
	</div>
	{#if reporting.length > 1}
		<div class="flex flex-wrap items-center gap-2">
			<span class="text-xs text-muted-foreground">Follow</span>
			{#each reporting as h (h.name)}
				<Button
					variant={prefs.followed?.host === h.name ? 'default' : 'outline'}
					size="xs"
					onclick={() => prefs.setWallHost(h.name, live.hosts)}>{h.name}</Button
				>
			{/each}
		</div>
	{/if}
	<p class="text-xs text-muted-foreground">
		Wallpaper takes the colours a machine generates from its wallpaper (DankMaterialShell's, else
		pywal's <code class="font-mono">wal/colors.json</code>) and changes with them. pywal's palette
		has one mode, so it decides light or dark.
	</p>
</section>
