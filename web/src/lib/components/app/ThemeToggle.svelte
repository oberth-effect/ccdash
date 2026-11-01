<script lang="ts">
	// The top bar's theme menu: light, dark or the system's, and the colour
	// scheme. A scheme with one mode wears it whatever is picked here.
	import MonitorIcon from '@lucide/svelte/icons/monitor';
	import MoonIcon from '@lucide/svelte/icons/moon';
	import PaletteIcon from '@lucide/svelte/icons/palette';
	import SunIcon from '@lucide/svelte/icons/sun';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { prefs, type Theme } from '#lib/prefs.svelte.js';
	import { SCHEMES, WALLPAPER } from '#lib/theme.js';
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}
			<Button variant="ghost" size="icon" aria-label="Theme" title="Theme" {...props}>
				{#if prefs.scheme !== 'neutral'}
					<PaletteIcon />
				{:else if prefs.theme === 'light'}
					<SunIcon />
				{:else if prefs.theme === 'dark'}
					<MoonIcon />
				{:else}
					<MonitorIcon />
				{/if}
			</Button>
		{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end" class="max-h-[80vh] w-48 overflow-y-auto">
		<DropdownMenu.RadioGroup value={prefs.theme} onValueChange={(v) => prefs.setTheme(v as Theme)}>
			<DropdownMenu.GroupHeading>Mode</DropdownMenu.GroupHeading>
			<DropdownMenu.RadioItem value="auto" closeOnSelect={false}>System</DropdownMenu.RadioItem>
			<DropdownMenu.RadioItem value="light" closeOnSelect={false}>Light</DropdownMenu.RadioItem>
			<DropdownMenu.RadioItem value="dark" closeOnSelect={false}>Dark</DropdownMenu.RadioItem>
		</DropdownMenu.RadioGroup>
		<DropdownMenu.Separator />
		<DropdownMenu.RadioGroup value={prefs.scheme} onValueChange={(v) => prefs.setScheme(v)}>
			<DropdownMenu.GroupHeading>Colours</DropdownMenu.GroupHeading>
			<DropdownMenu.RadioItem value={WALLPAPER} closeOnSelect={false}
				>Wallpaper</DropdownMenu.RadioItem
			>
			{#each SCHEMES as s (s.id)}
				<DropdownMenu.RadioItem value={s.id} closeOnSelect={false}>{s.label}</DropdownMenu.RadioItem
				>
			{/each}
		</DropdownMenu.RadioGroup>
	</DropdownMenu.Content>
</DropdownMenu.Root>
