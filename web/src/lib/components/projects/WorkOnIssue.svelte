<script lang="ts">
	// Opens the start sheet for a session on an open issue, in the issue's
	// workspace and project. `onstart` closes what the sheet would cover.
	import PlayIcon from '@lucide/svelte/icons/play';
	import { Button } from '#lib/components/ui/button/index.js';
	import { live } from '#lib/live/store.svelte.js';
	import { starter } from '#lib/starter.svelte.js';

	let {
		ws,
		issue,
		size = 'xs',
		onstart
	}: {
		ws: string;
		issue: { slug: string; title: string; project: string | null };
		size?: 'xs' | 'sm';
		onstart?: () => void;
	} = $props();

	const connected = $derived(live.hosts.some((h) => h.connected));
</script>

<Button
	variant="outline"
	{size}
	disabled={!connected}
	title={connected ? 'Start a session on this issue' : 'No host is connected'}
	onclick={() => {
		onstart?.();
		starter.show({
			ws,
			project: issue.project,
			issue: { slug: issue.slug, title: issue.title }
		});
	}}
>
	<PlayIcon /> Work on this
</Button>
