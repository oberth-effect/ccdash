<script lang="ts">
	// Server-rendered markdown (src/markdown.rs, raw HTML escaped), folded
	// when long with a button to unfold; its code blocks become `CodeBlock`s;
	// `note:` links (`[[slug]]` in notes) go to `onnote` instead of a new tab.
	import { mount, unmount, untrack } from 'svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import CodeBlock from './CodeBlock.svelte';

	let {
		html,
		clamp = false,
		onnote
	}: { html: string; clamp?: boolean; onnote?: (slug: string) => void } = $props();

	let el = $state<HTMLDivElement | null>(null);
	// svelte-ignore state_referenced_locally
	let folded = $state(clamp);
	let overflows = $state(false);

	// NOTE: a new `item` with the same text leaves `{@html}` alone: only a new text is new blocks
	const source = $derived(html);

	$effect(() => {
		void source;
		const root = el;
		if (!root) return;
		const blocks = untrack(() =>
			[...root.querySelectorAll('pre')].map((pre) => {
				const code = pre.querySelector('code');
				const lang = code?.className.match(/language-(\S+)/)?.[1] ?? null;
				const text = (code ?? pre).textContent ?? '';
				// NOTE: the `pre` stays, emptied: `{@html}` removes the nodes it inserted by identity
				pre.className = 'code-host';
				pre.replaceChildren();
				return mount(CodeBlock, { target: pre, props: { code: text, lang } });
			})
		);
		return () => blocks.forEach((block) => void unmount(block));
	});

	$effect(() => {
		if (!el) return;
		for (const a of el.querySelectorAll('a')) {
			const note = a.getAttribute('href')?.match(/^note:(.+)$/);
			if (note && onnote) {
				a.onclick = (event) => {
					event.preventDefault();
					onnote(note[1]);
				};
				continue;
			}
			a.target = '_blank';
			a.rel = 'noopener noreferrer';
		}
		overflows = folded && el.scrollHeight > el.clientHeight + 8;
		if (folded && !overflows) folded = false;
	});
</script>

<div bind:this={el} class="prose-chat {folded ? 'clamp' : ''}">
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html html}
</div>
{#if folded && overflows}
	<Button variant="link" size="xs" class="h-auto px-0" onclick={() => (folded = false)}
		>Show all</Button
	>
{/if}
