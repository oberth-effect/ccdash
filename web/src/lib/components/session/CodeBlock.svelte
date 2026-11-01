<script lang="ts">
	// A block of code: syntax colours (plain until they load, and for what
	// `highlight` does not know), line numbers beside the text, the language
	// and a copy button. A single line carries the button at its end instead
	// of a bar above it.
	import CheckIcon from '@lucide/svelte/icons/check';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import XIcon from '@lucide/svelte/icons/x';
	import { highlight } from '#lib/highlight.js';
	import { cn, copyText } from '#lib/utils.js';

	let {
		code,
		lang = null,
		class: className,
		bodyClass
	}: { code: string; lang?: string | null; class?: string; bodyClass?: string } = $props();

	const COPIED_MS = 1500;

	const text = $derived(code.replace(/\n$/, ''));
	const lines = $derived(text.split('\n').length);
	const gutter = $derived(Array.from({ length: lines }, (_, i) => i + 1).join('\n'));
	const bar = $derived(lines > 1 && Boolean(lang));

	let coloured = $state<string | null>(null);
	let copied = $state<boolean | null>(null);
	let timer: ReturnType<typeof setTimeout> | undefined;

	$effect(() => {
		let stale = false;
		coloured = null;
		highlight(text, lang).then((html) => {
			if (!stale) coloured = html;
		});
		return () => {
			stale = true;
		};
	});

	async function copy() {
		copied = await copyText(text);
		clearTimeout(timer);
		timer = setTimeout(() => (copied = null), COPIED_MS);
	}
</script>

<figure
	class={cn(
		'code-block relative my-2 overflow-hidden rounded-md border bg-muted/60 font-mono text-[0.8rem] leading-[1.45] whitespace-normal',
		className
	)}
>
	{#if bar}
		<figcaption class="border-b bg-muted/60 px-3 py-0.5 text-xs text-muted-foreground">
			{lang}
		</figcaption>
	{/if}
	<button
		type="button"
		class="absolute right-1 z-[1] inline-flex size-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-foreground {bar
			? 'top-0'
			: 'top-1.5'}"
		aria-label={copied === false ? 'Copying failed' : copied ? 'Copied' : 'Copy'}
		title={copied === false ? 'The browser refused the clipboard' : 'Copy'}
		onclick={copy}
	>
		{#if copied === false}<XIcon class="size-3.5 text-critical" />
		{:else if copied}<CheckIcon class="size-3.5 text-your-turn" />
		{:else}<CopyIcon class="size-3.5" />{/if}
	</button>
	<div class={cn('flex overflow-y-auto', bodyClass)}>
		{#if lines > 1}
			<span
				class="shrink-0 border-r px-2 py-2 text-right whitespace-pre text-muted-foreground/70 select-none"
				aria-hidden="true">{gutter}</span
			>
		{/if}
		<!-- eslint-disable svelte/no-at-html-tags -->
		<pre class="min-w-0 flex-1 overflow-x-auto py-2 pr-8 pl-3"><code
				>{#if coloured !== null}{@html coloured}{:else}{text}{/if}</code
			></pre>
		<!-- eslint-enable svelte/no-at-html-tags -->
	</div>
</figure>
