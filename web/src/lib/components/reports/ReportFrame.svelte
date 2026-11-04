<script lang="ts">
	// A published report in a sandboxed frame. The hub hands out a view link
	// (an hour) that needs no token, so the frame can load it; the document
	// is an opaque origin that runs its own scripts and loads nothing from
	// the network. The report follows the panel's light or dark mode through
	// a message; an expired link is minted again on reload.
	import { untrack } from 'svelte';
	import { Unauthorized } from '#lib/api/client.js';
	import { mintReportView } from '#lib/api/index.js';
	import { live } from '#lib/live/store.svelte.js';
	import { prefs } from '#lib/prefs.svelte.js';

	let {
		ws,
		slug,
		/** Bump to load the report again (a new version). */
		reload = 0,
		class: className = ''
	}: { ws: string; slug: string; reload?: number; class?: string } = $props();

	let url = $state('');
	let error = $state('');
	let frame = $state<HTMLIFrameElement | null>(null);

	async function mint() {
		const target = `${ws}/${slug}/${reload}`;
		url = '';
		error = '';
		try {
			const { url: link } = await mintReportView(ws, slug);
			if (`${ws}/${slug}/${reload}` !== target) return;
			url = link;
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		}
	}

	$effect(() => {
		void ws;
		void slug;
		void reload;
		untrack(mint);
	});

	/** The mode the panel shows, as the report should. */
	const mode = $derived.by(() => {
		if (prefs.theme !== 'auto') return prefs.theme;
		return typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: dark)').matches
			? 'dark'
			: 'light';
	});

	function tell() {
		// NOTE: the frame is an opaque origin, so the target is `*`; the message carries nothing secret
		frame?.contentWindow?.postMessage({ ccdash: 'theme', mode }, '*');
	}

	$effect(() => {
		void mode;
		tell();
	});
</script>

{#if error}
	<p class="p-4 text-sm text-critical" role="alert">{error}</p>
{:else if !url}
	<p class="p-4 text-sm text-muted-foreground">Loading…</p>
{:else}
	<iframe
		bind:this={frame}
		src={url}
		title="Report"
		class="block h-full w-full border-0 bg-background {className}"
		sandbox="allow-scripts allow-popups allow-popups-to-escape-sandbox allow-downloads"
		referrerpolicy="no-referrer"
		onload={tell}
	></iframe>
{/if}
