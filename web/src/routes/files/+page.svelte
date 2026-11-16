<script lang="ts">
	// Files: everything attached to prompts and still kept on the hub, with
	// what it takes of the quota. Each file shows where it came from, how
	// big it is and when the hub drops it; Keep pins it past that, Delete
	// takes it away now. The kind and the search are in the address.
	import DownloadIcon from '@lucide/svelte/icons/download';
	import FileIcon from '@lucide/svelte/icons/file';
	import MicIcon from '@lucide/svelte/icons/mic';
	import PinIcon from '@lucide/svelte/icons/pin';
	import PinOffIcon from '@lucide/svelte/icons/pin-off';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import VideoIcon from '@lucide/svelte/icons/video';
	import { page } from '$app/state';
	import { Unauthorized } from '#lib/api/client.js';
	import {
		deleteAttachment,
		fetchAttachments,
		pinAttachment,
		type Attachment,
		type Attachments
	} from '#lib/api/index.js';
	import { forget } from '#lib/attachments.svelte.js';
	import Meter from '#lib/components/app/Meter.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { fmtBytes, fmtDate, fmtDuration, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { label } from '#lib/session.js';

	const KINDS = [
		['', 'All'],
		['image', 'Pictures'],
		['audio', 'Sound'],
		['video', 'Video'],
		['file', 'Other']
	] as const;
	const MIB = 1024 * 1024;
	const DAY_MS = 86_400_000;

	let data = $state<Attachments | null>(null);
	let error = $state('');
	/** The file asked to be deleted, until confirmed or left. */
	let confirming = $state<string | null>(null);

	const kind = $derived(page.url.searchParams.get('kind') ?? '');
	const q = $derived(page.url.searchParams.get('q') ?? '');
	const shown = $derived(
		(data?.attachments ?? []).filter(
			(a) => (!kind || a.kind === kind) && (!q || a.name.toLowerCase().includes(q.toLowerCase()))
		)
	);
	const quota = $derived((data?.limits.quota_mb ?? 0) * MIB);
	const used = $derived(data && quota ? (data.total_bytes / quota) * 100 : 0);

	function setParam(name: string, value: string) {
		const url = new URL(page.url.href);
		if (value) url.searchParams.set(name, value);
		else url.searchParams.delete(name);
		history.replaceState(history.state, '', url);
	}

	async function load() {
		try {
			data = await fetchAttachments();
			error = '';
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		}
	}

	$effect(() => {
		if (live.ready) load();
	});

	async function act(run: () => Promise<unknown>) {
		try {
			await run();
			await load();
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		}
	}

	function remove(a: Attachment) {
		confirming = null;
		act(async () => {
			await deleteAttachment(a.id);
			forget(a.id);
		});
	}

	function fate(a: Attachment): string {
		if (a.pinned) return 'kept until you delete it';
		if (!a.expires_at) return 'kept while there is room';
		const left = a.expires_at - live.now;
		return left > DAY_MS
			? `dropped in ${plural(Math.floor(left / DAY_MS), 'day')}`
			: left > 0
				? `dropped in ${fmtDuration(left)}`
				: 'dropped at the next look';
	}

	function from(a: Attachment): string {
		if (!a.session) return 'no session';
		const s = live.sessions.get(a.session);
		return s ? label(s) : `session ${a.session.slice(0, 8)}`;
	}
</script>

<div class="mx-auto flex max-w-6xl flex-col gap-4 p-3 sm:p-4">
	<header class="flex flex-col gap-2">
		<div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
			<h2 class="text-lg font-semibold">Files</h2>
			{#if data}
				<span class="text-sm text-muted-foreground">
					{plural(data.attachments.length, 'file')} · {fmtBytes(data.total_bytes)}{quota
						? ` of ${fmtBytes(quota)}`
						: ''}
				</span>
				{#if quota}<Meter pct={used} label="of the quota" />{/if}
			{/if}
		</div>
		<p class="max-w-3xl text-xs text-muted-foreground">
			What you attached to prompts: a session's machine fetches a file from here when the prompt
			reaches it.
			{#if data}
				{data.limits.retention_days
					? `The hub drops a file ${plural(data.limits.retention_days, 'day')} after its upload unless you keep it`
					: 'The hub keeps files until you delete them'}{quota
					? '; when the store is full the oldest unkept ones go first.'
					: '.'}
				Up to {data.limits.max_image_mb} MB a picture, {data.limits.max_audio_mb} MB a recording,
				{data.limits.max_video_mb} MB a video, {data.limits.max_file_mb} MB anything else.
			{/if}
		</p>
		<div class="flex flex-wrap items-center gap-2">
			<div class="inline-flex w-fit gap-0.5 rounded-lg bg-muted p-[3px] text-sm" role="group">
				{#each KINDS as [value, name] (value)}
					<button
						type="button"
						class="rounded-md px-2.5 py-1 {kind === value
							? 'bg-background font-medium shadow-sm'
							: 'text-muted-foreground hover:text-foreground'}"
						aria-pressed={kind === value}
						onclick={() => setParam('kind', value)}>{name}</button
					>
				{/each}
			</div>
			<Input
				value={q}
				type="search"
				placeholder="Find by name…"
				aria-label="Find a file by name"
				class="h-8 max-w-56"
				oninput={(e) => setParam('q', e.currentTarget.value)}
			/>
		</div>
		{#if error}<p class="text-sm text-critical" role="alert">{error}</p>{/if}
	</header>

	{#if !data && !error}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{:else if data && !shown.length}
		<p class="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground">
			{#if data.attachments.length}
				No file matches.
			{:else}
				Nothing is kept. Attach a photo or a file to a prompt with the + in a session's message
				field, or paste a screenshot into it.
			{/if}
		</p>
	{:else}
		<ul class="grid grid-cols-[repeat(auto-fill,minmax(15rem,1fr))] gap-3">
			{#each shown as a (a.id)}
				<li class="flex min-w-0 flex-col overflow-hidden rounded-lg border bg-card">
					<a
						href={a.url}
						target="_blank"
						rel="noopener noreferrer"
						class="flex h-36 items-center justify-center bg-muted"
						title="Open {a.name}"
					>
						{#if a.kind === 'image'}
							<img src={a.url} alt={a.name} loading="lazy" class="size-full object-contain" />
						{:else if a.kind === 'audio'}
							<MicIcon class="size-8 text-muted-foreground" />
						{:else if a.kind === 'video'}
							<VideoIcon class="size-8 text-muted-foreground" />
						{:else}
							<FileIcon class="size-8 text-muted-foreground" />
						{/if}
					</a>
					<div class="flex min-w-0 flex-col gap-0.5 p-2.5">
						<span class="truncate text-sm font-medium" title={a.name}>{a.name}</span>
						<span class="truncate text-xs text-muted-foreground">
							{fmtBytes(a.bytes)} · {fmtDate(a.created_at)} ·
							{#if a.session && live.sessions.has(a.session)}
								<a class="underline" href="/sessions/{a.session}">{from(a)}</a>
							{:else}{from(a)}{/if}
						</span>
						<span class="text-xs {a.pinned ? 'text-your-turn' : 'text-muted-foreground'}">
							{a.fetched_at ? 'Sent' : 'Never sent'} · {fate(a)}
						</span>
					</div>
					<div class="mt-auto flex items-center gap-1 border-t px-1.5 py-1">
						{#if confirming === a.id}
							<span class="px-1 text-xs">Delete it for good?</span>
							<Button variant="destructive" size="xs" class="ml-auto" onclick={() => remove(a)}
								>Delete</Button
							>
							<Button variant="ghost" size="xs" onclick={() => (confirming = null)}>Keep</Button>
						{:else}
							<Button
								variant="ghost"
								size="xs"
								aria-pressed={a.pinned}
								title={a.pinned
									? 'Let the hub drop it when its time comes'
									: 'Keep it past its time and the quota'}
								onclick={() => act(() => pinAttachment(a.id, !a.pinned))}
							>
								{#if a.pinned}<PinOffIcon /> Unkeep{:else}<PinIcon /> Keep{/if}
							</Button>
							<Button variant="ghost" size="xs" href={a.url} download={a.name}>
								<DownloadIcon /> Download
							</Button>
							<Button
								variant="ghost"
								size="icon-xs"
								class="ml-auto text-critical"
								aria-label="Delete {a.name}"
								title="Delete"
								onclick={() => (confirming = a.id)}
							>
								<Trash2Icon />
							</Button>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>
