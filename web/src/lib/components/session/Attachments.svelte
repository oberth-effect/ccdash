<script lang="ts">
	// A prompt's attachments in the chat: pictures as thumbnails that open
	// large, recordings and videos with a player, other files as a chip to
	// download. One the hub no longer has, or that did not reach the
	// session's machine, says so instead.
	import FileIcon from '@lucide/svelte/icons/file';
	import ImageOffIcon from '@lucide/svelte/icons/image-off';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import { Unauthorized } from '#lib/api/client.js';
	import type { Attachment } from '#lib/api/types.js';
	import { attachmentOf } from '#lib/attachments.svelte.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { fmtBytes } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';

	let {
		ids,
		failed = {},
		made = {},
		align = 'start'
	}: {
		ids: string[];
		/** Why an attachment did not reach the session's machine, by id. */
		failed?: Record<string, string>;
		/** What Claude got in a recording's or a video's stead, by id. */
		made?: Record<string, string[]>;
		align?: 'start' | 'end';
	} = $props();

	/** undefined: loading; null: the hub no longer has it. */
	let rows = $state<Record<string, Attachment | null>>({});
	let open = $state<Attachment | null>(null);

	$effect(() => {
		for (const id of ids) {
			if (id in rows) continue;
			attachmentOf(id)
				.then((a) => (rows[id] = a))
				.catch((e) => {
					if (e instanceof Unauthorized) live.needToken = true;
				});
		}
	});
</script>

<div class="flex max-w-full flex-wrap gap-2 {align === 'end' ? 'justify-end' : ''}">
	{#each ids as id (id)}
		{@const a = rows[id]}
		<div class="flex max-w-full min-w-0 flex-col gap-1">
			{#if a === undefined}
				<div class="h-24 w-32 animate-pulse rounded-lg bg-muted" aria-label="Loading"></div>
			{:else if a === null}
				<div
					class="flex items-center gap-2 rounded-lg border border-dashed px-3 py-2 text-xs text-muted-foreground"
				>
					<ImageOffIcon class="size-4 shrink-0" /> An attachment the hub no longer keeps
				</div>
			{:else if a.kind === 'image' && a.url}
				<button
					type="button"
					class="overflow-hidden rounded-lg border bg-muted focus-visible:ring-2 focus-visible:ring-ring/50"
					title="{a.name} · {fmtBytes(a.bytes)}"
					onclick={() => (open = a)}
				>
					<img src={a.url} alt={a.name} loading="lazy" class="max-h-40 max-w-full object-contain" />
				</button>
			{:else if a.kind === 'audio' && a.url}
				<audio controls preload="metadata" src={a.url} class="h-10 max-w-full"></audio>
				<span class="text-xs text-muted-foreground">{a.name} · {fmtBytes(a.bytes)}</span>
			{:else if a.kind === 'video' && a.url}
				<!-- svelte-ignore a11y_media_has_caption -->
				<video
					controls
					preload="metadata"
					src={a.url}
					class="max-h-56 max-w-full rounded-lg border bg-black"
				></video>
				<span class="text-xs text-muted-foreground">{a.name} · {fmtBytes(a.bytes)}</span>
			{:else}
				<a
					href={a.url}
					download={a.name}
					class="flex items-center gap-2 rounded-lg border bg-card px-3 py-2 text-sm hover:bg-accent/50"
				>
					<FileIcon class="size-4 shrink-0 text-muted-foreground" />
					<span class="truncate">{a.name}</span>
					<span class="shrink-0 text-xs text-muted-foreground">{fmtBytes(a.bytes)}</span>
				</a>
			{/if}
			{#each made[id] ?? [] as line (line)}
				<p class="max-w-xs text-xs text-muted-foreground">{line}</p>
			{/each}
			{#if failed[id]}
				<p class="flex max-w-xs items-start gap-1 text-xs text-warning" role="alert">
					<TriangleAlertIcon class="mt-0.5 size-3 shrink-0" />
					Claude did not get this file: {failed[id]}
				</p>
			{/if}
		</div>
	{/each}
</div>

<Dialog.Root
	open={open !== null}
	onOpenChange={(o) => {
		if (!o) open = null;
	}}
>
	<Dialog.Content
		class="flex max-h-[94dvh] w-auto max-w-[96vw] flex-col gap-2 p-2 sm:max-w-[96vw] sm:p-3"
	>
		{#if open}
			<Dialog.Title class="truncate pr-8 text-sm">{open.name}</Dialog.Title>
			<Dialog.Description class="sr-only">The attached picture, in full.</Dialog.Description>
			<img
				src={open.url}
				alt={open.name}
				class="min-h-0 max-w-full flex-1 rounded-md object-contain"
			/>
			<p class="flex flex-wrap items-center gap-x-3 text-xs text-muted-foreground">
				<span>{fmtBytes(open.bytes)}</span>
				<a class="underline" href={open.url} download={open.name}>Download</a>
				<a class="underline" href="/files">All files</a>
			</p>
		{/if}
	</Dialog.Content>
</Dialog.Root>
