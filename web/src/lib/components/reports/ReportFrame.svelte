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

	/** The report's variables, from the panel's: the report wears the
	 * panel's scheme (Neutral, Catppuccin, the wallpaper's…) while framed. */
	const VARS: Record<string, string> = {
		bg: '--background',
		surface: '--card',
		'surface-2': '--muted',
		fg: '--foreground',
		muted: '--muted-foreground',
		border: '--border',
		'border-strong': '--input',
		accent: '--primary',
		'accent-soft': '--accent',
		'accent-fg': '--primary-foreground',
		good: '--good',
		warn: '--warning',
		bad: '--critical',
		info: '--working',
		'series-1': '--working',
		'series-2': '--permission',
		'series-3': '--your-turn',
		'series-4': '--series-4',
		'series-5': '--series-5',
		'series-6': '--series-6',
		'seq-1': '--seq-1',
		'seq-2': '--seq-2',
		'seq-3': '--seq-3',
		'seq-4': '--seq-4',
		'seq-5': '--seq-5',
		'seq-6': '--seq-6',
		'hl-keyword': '--hl-keyword',
		'hl-string': '--hl-string',
		'hl-number': '--hl-number',
		'hl-title': '--hl-title',
		'hl-comment': '--hl-comment'
	};

	function vars(): Record<string, string> {
		const computed = getComputedStyle(document.documentElement);
		const out: Record<string, string> = {};
		for (const [name, source] of Object.entries(VARS)) {
			const value = computed.getPropertyValue(source).trim();
			if (value) out[name] = value;
		}
		return out;
	}

	function tell() {
		// NOTE: the frame is an opaque origin, so the target is `*`; the message carries nothing secret
		frame?.contentWindow?.postMessage({ ccdash: 'theme', mode, vars: vars() }, '*');
	}

	$effect(() => {
		void mode;
		void prefs.scheme;
		void prefs.followed;
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
