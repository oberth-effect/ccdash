<script lang="ts">
	// A report: the framed document filling the page, with its note (the
	// abstract, where it came from) in a drawer beside it; Download keeps
	// the HTML file to send on, Open shows it in a tab of its own.
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
	import PanelRightIcon from '@lucide/svelte/icons/panel-right';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import { Unauthorized } from '#lib/api/client.js';
	import { fetchNote, fetchReportFile, mintReportView, type NotePage } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import ReportFrame from '#lib/components/reports/ReportFrame.svelte';
	import StatusBadge from '#lib/components/notes/StatusBadge.svelte';
	import Prose from '#lib/components/session/Prose.svelte';
	import { fmtBytes, fmtStamp } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { noteHref, projectHref } from '#lib/notes.js';
	import { notePanel } from '#lib/notepanel.svelte.js';

	const ws = $derived(page.params.ws!);
	const slug = $derived(page.params.slug!);

	let note = $state<NotePage | null>(null);
	let error = $state('');
	let reload = $state(0);
	let aside = $state(false);
	let busy = $state('');
	let actionError = $state('');

	async function load() {
		const target = `${ws}/${slug}`;
		try {
			const n = await fetchNote(ws, slug);
			if (`${ws}/${slug}` !== target) return;
			if (n.kind !== 'report') {
				error = `${slug} is a ${n.kind}, not a report`;
				return;
			}
			note = n;
			error = '';
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		}
	}

	$effect(() => {
		void ws;
		void slug;
		untrack(() => {
			note = null;
			error = '';
			load();
		});
	});

	// a new version published: the note and the frame follow
	$effect(() => {
		const changed = live.notes;
		untrack(() => {
			if (changed.n && changed.workspace === ws) {
				load().then(() => reload++);
			}
		});
	});

	async function run(what: string, action: () => Promise<void>) {
		busy = what;
		actionError = '';
		try {
			await action();
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else actionError = (e as Error).message;
		} finally {
			busy = '';
		}
	}

	const download = () =>
		run('download', async () => {
			const blob = await fetchReportFile(ws, slug);
			const link = document.createElement('a');
			link.href = URL.createObjectURL(blob);
			link.download = `${slug}.html`;
			link.click();
			URL.revokeObjectURL(link.href);
		});

	const openTab = () =>
		run('open', async () => {
			const { url } = await mintReportView(ws, slug);
			window.open(url, '_blank', 'noopener');
		});

	const bytes = $derived(typeof note?.extra.bytes === 'number' ? note.extra.bytes : null);
	const host = $derived(typeof note?.extra.host === 'string' ? note.extra.host : null);
	const path = $derived(typeof note?.extra.path === 'string' ? note.extra.path : null);
</script>

<svelte:head>
	<title>{note?.title ?? 'Report'} · Reports · ccdash</title>
</svelte:head>

<div class="flex h-full min-h-0 flex-col">
	<header class="flex flex-wrap items-center gap-2 border-b px-3 py-2">
		<Button variant="ghost" size="icon-sm" href="/reports" aria-label="All reports" title="Reports">
			<ArrowLeftIcon />
		</Button>
		<div class="flex min-w-0 flex-1 flex-col">
			<div class="flex min-w-0 items-center gap-2">
				<h2 class="truncate text-sm font-semibold">{note?.title ?? slug}</h2>
				{#if note && note.status !== 'active'}<StatusBadge {note} />{/if}
			</div>
			{#if note}
				<p class="truncate text-xs text-muted-foreground">
					<a class="hover:underline" href={projectHref(note.workspace, note.project)}
						>{note.workspace}{note.project ? ` / ${note.project}` : ''}</a
					>
					· {fmtStamp(note.updated_at)}
					{#if bytes !== null}· {fmtBytes(bytes)}{/if}
					{#if host}· from {host}{/if}
					{#if note.version > 1}· v{note.version}{/if}
				</p>
			{/if}
		</div>
		{#if actionError}<span class="text-xs text-critical" role="alert">{actionError}</span>{/if}
		<Button
			variant="outline"
			size="sm"
			onclick={download}
			disabled={!note || busy === 'download'}
			title="Keep the HTML file, to send on"
		>
			<DownloadIcon /> <span class="hidden sm:inline">Download</span>
		</Button>
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={openTab}
			disabled={!note || busy === 'open'}
			aria-label="Open in a new tab"
			title="Open in a new tab"
		>
			<ExternalLinkIcon />
		</Button>
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={() => reload++}
			aria-label="Load again"
			title="Load again"
		>
			<RefreshCwIcon />
		</Button>
		<Button
			variant={aside ? 'secondary' : 'ghost'}
			size="icon-sm"
			aria-pressed={aside}
			onclick={() => (aside = !aside)}
			aria-label="The abstract"
			title="The abstract"
		>
			<PanelRightIcon />
		</Button>
	</header>
	{#if error}
		<p class="p-4 text-sm text-critical" role="alert">{error}</p>
	{:else}
		<div class="flex min-h-0 flex-1">
			<div class="min-h-0 min-w-0 flex-1">
				<ReportFrame {ws} {slug} {reload} />
			</div>
			{#if aside}
				<aside class="flex w-full max-w-sm flex-col gap-3 overflow-y-auto border-l p-4 text-sm">
					{#if !note}
						<p class="text-muted-foreground">Loading…</p>
					{:else}
						<h3 class="text-xs font-semibold text-muted-foreground uppercase">Abstract</h3>
						{#if note.summary}<p class="text-muted-foreground italic">{note.summary}</p>{/if}
						<Prose html={note.html} onnote={(target) => notePanel.show(ws, target)} />
						<dl class="grid grid-cols-[max-content_1fr] gap-x-3 gap-y-1 text-xs">
							<dt class="text-muted-foreground">Slug</dt>
							<dd><code class="font-mono">{note.slug}</code></dd>
							<dt class="text-muted-foreground">Published</dt>
							<dd>{fmtStamp(note.updated_at)}</dd>
							{#if host}<dt class="text-muted-foreground">From</dt>
								<dd>{host}</dd>{/if}
							{#if path}<dt class="text-muted-foreground">Was at</dt>
								<dd class="font-mono break-all">{path}</dd>{/if}
							{#if bytes !== null}<dt class="text-muted-foreground">Size</dt>
								<dd>{fmtBytes(bytes)}</dd>{/if}
							{#if note.created_by}<dt class="text-muted-foreground">Session</dt>
								<dd>
									<a class="hover:underline" href="/sessions/{note.created_by}"
										>{note.created_by.slice(0, 8)}</a
									>
								</dd>{/if}
						</dl>
						<div class="flex flex-wrap gap-2 border-t pt-3">
							<Button size="sm" variant="outline" href={noteHref(ws, slug)}>The note</Button>
							<Button
								size="sm"
								variant="ghost"
								href="{projectHref(ws, note.project)}?tab=graph&note={encodeURIComponent(slug)}"
								>In the graph</Button
							>
						</div>
					{/if}
				</aside>
			{/if}
		</div>
	{/if}
</div>
