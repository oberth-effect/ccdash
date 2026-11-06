<script lang="ts">
	// One past session, enough to tell it is the one and to pick it up: its
	// title, where and when it ran and its numbers; Resume (the start sheet,
	// on a machine of your choice), its page when the hub has the chat, the
	// terminal command. Overview: Claude Code's recap, what you asked first,
	// where it ended, where the search matched, every prompt, the files it
	// edited. Conversation: your prompts and Claude's text in order (tool
	// calls left out), paged, opened at a match or a prompt.
	import { tick, untrack } from 'svelte';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import CheckIcon from '@lucide/svelte/icons/check';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
	import GitBranchIcon from '@lucide/svelte/icons/git-branch';
	import PlayIcon from '@lucide/svelte/icons/play';
	import XIcon from '@lucide/svelte/icons/x';
	import { Unauthorized } from '#lib/api/client.js';
	import {
		fetchPastConversation,
		fetchPastSession,
		type PastConversation,
		type PastDetail
	} from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import Prose from '#lib/components/session/Prose.svelte';
	import { fmtBytes, fmtDuration, fmtUsd, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import {
		cannotResume,
		fmtAgo,
		fmtFull,
		fmtWhenShort,
		headless,
		highlightTerms,
		marks,
		modelName,
		pastTitle,
		resumeCommand
	} from '#lib/past.js';
	import { resumablePast, starter } from '#lib/starter.svelte.js';
	import { hostColor } from '#lib/workspace.js';

	let {
		id,
		q,
		hosts,
		wide,
		onclose
	}: {
		id: string;
		/** The list's search: its words are marked, its matches listed. */
		q: string;
		/** Every machine of the list, for stable colours. */
		hosts: string[];
		/** Beside the list (else it is the page, with a way back). */
		wide: boolean;
		onclose: () => void;
	} = $props();

	const CONVERSATION_PAGE = 40;
	const ROLE: Record<string, string> = { you: 'You', claude: 'Claude', title: 'Title' };

	let detail = $state<PastDetail | null>(null);
	let error = $state('');
	let tab = $state<'overview' | 'conversation'>('overview');
	let conv = $state<PastConversation | null>(null);
	let convError = $state('');
	let copied = $state<string | null>(null);
	let root = $state<HTMLElement | null>(null);
	let allPrompts = $state(false);
	let fullAsk = $state(false);
	/** The text a match or a prompt opened the conversation at. */
	let target = $state<number | null>(null);

	const s = $derived(detail?.session);
	const why = $derived(s ? cannotResume(s) : null);
	const machineOnline = (h: string) => live.hosts.some((x) => x.name === h && x.connected);

	// A new session (or search): load it, back to its overview.
	$effect(() => {
		const [want, words] = [id, q];
		let stale = false;
		untrack(() => {
			detail = null;
			error = '';
			conv = null;
			tab = 'overview';
			allPrompts = false;
		});
		fetchPastSession(want, words || undefined)
			.then((d) => {
				if (!stale) detail = d;
			})
			.catch((e) => {
				if (stale) return;
				if (e instanceof Unauthorized) live.needToken = true;
				else error = (e as Error).message;
			});
		return () => (stale = true);
	});

	// NOTE: after every render of new text: the ranges hold the nodes they were made on
	$effect(() => {
		void [detail, conv, tab, allPrompts];
		const el = root;
		const words = q;
		if (!el || !words) return;
		let undo = () => {};
		tick().then(() => (undo = highlightTerms(el, words)));
		return () => undo();
	});

	async function openConversation(at: { around?: number; offset?: number } = {}) {
		tab = 'conversation';
		target = at.around ?? null;
		convError = '';
		try {
			conv = await fetchPastConversation(id, { ...at, limit: CONVERSATION_PAGE });
			await tick();
			if (at.around !== undefined)
				root
					?.querySelector(`[data-text="${at.around}"]`)
					?.scrollIntoView({ block: 'center', behavior: 'smooth' });
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else convError = (e as Error).message;
		}
	}

	async function copy(what: string, text: string) {
		await navigator.clipboard?.writeText(text);
		copied = what;
		setTimeout(() => {
			if (copied === what) copied = null;
		}, 1500);
	}

	function resume() {
		if (s && !why) starter.show({ resume: resumablePast(s) });
	}

	export function resumeNow() {
		resume();
	}

	const span = $derived(s ? s.last_at - s.first_at : 0);
	const facts = $derived.by((): [string, string][] => {
		if (!s) return [];
		const out: [string, string][] = [
			['Started', fmtFull(s.first_at)],
			['Last active', `${fmtFull(s.last_at)} (${fmtAgo(s.last_at, live.now)})`],
			[
				'Length',
				[
					span > 60_000 ? `over ${fmtDuration(span)}` : null,
					s.active_ms > 0 ? `${fmtDuration(s.active_ms)} of Claude working` : null
				]
					.filter(Boolean)
					.join(', ') || '—'
			],
			[
				'Size',
				[
					fmtBytes(s.size),
					plural(s.prompts, 'prompt'),
					plural(s.replies, 'reply', 'replies'),
					plural(s.tools, 'tool call'),
					s.compactions ? plural(s.compactions, 'compaction') : null
				]
					.filter(Boolean)
					.join(' · ')
			]
		];
		const cost = [
			s.cost_usd ? fmtUsd(s.cost_usd) : null,
			s.lines_added || s.lines_removed
				? `+${s.lines_added ?? 0} −${s.lines_removed ?? 0} lines`
				: null
		].filter(Boolean);
		if (cost.length) out.push(['Cost', cost.join(' · ')]);
		const models = (detail?.models ?? []).map(([m, n]) => `${modelName(m)} (${n})`);
		if (models.length) out.push(['Models', models.join(', ')]);
		const via = [
			s.version ? `Claude Code ${s.version}` : null,
			headless(s) ? `headless (${s.entrypoint})` : null
		].filter(Boolean);
		if (via.length) out.push(['Ran in', via.join(', ')]);
		return out;
	});
	const shownPrompts = $derived(
		allPrompts ? (detail?.prompts ?? []) : (detail?.prompts ?? []).slice(0, 12)
	);
</script>

<div class="flex min-w-0 flex-col gap-4 p-3 sm:p-4" bind:this={root}>
	<div class="flex items-start gap-2">
		{#if !wide}
			<Button variant="ghost" size="icon-sm" aria-label="Back to the list" onclick={onclose}>
				<ArrowLeftIcon />
			</Button>
		{/if}
		<div class="flex min-w-0 flex-1 flex-col gap-1">
			<h2 class="text-base leading-snug font-semibold">
				{s ? pastTitle(s) : error ? 'Not found' : 'Loading…'}
			</h2>
			{#if s}
				<span class="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
					<button
						type="button"
						class="inline-flex items-center gap-1 font-mono hover:text-foreground"
						title="Copy the session id"
						onclick={() => copy('id', s.id)}
					>
						{s.id.slice(0, 8)}
						{#if copied === 'id'}<CheckIcon class="size-3" />{:else}<CopyIcon class="size-3" />{/if}
					</button>
					{#if s.open}
						<span class="rounded-full bg-working/15 px-1.5 font-medium text-working"
							>running now</span
						>
					{/if}
					{#if s.continued_in}
						<a class="underline underline-offset-2" href="/sessions/past?id={s.continued_in}"
							>continued as {s.continued_in.slice(0, 8)}</a
						>
					{/if}
				</span>
			{/if}
		</div>
		{#if wide}
			<Button
				variant="ghost"
				size="icon-sm"
				aria-label="Close"
				title="Close (Esc)"
				onclick={onclose}
			>
				<XIcon />
			</Button>
		{/if}
	</div>

	{#if error}
		<p class="text-sm text-critical" role="alert">{error}</p>
	{:else if s && detail}
		<!-- where it ran -->
		<div class="flex flex-col gap-1.5 rounded-lg border bg-card p-3 text-xs">
			<div class="flex flex-wrap items-center gap-1.5">
				{#each s.hosts as h (h)}
					<span
						class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5"
						title={machineOnline(h) ? `${h} is connected` : `${h} is offline`}
					>
						<span class="size-2 rounded-full" style="background: {hostColor(hosts, h)}"></span>
						{h}
						{#if h === s.host && s.hosts.length > 1}<span class="text-muted-foreground">last</span
							>{/if}
						{#if !machineOnline(h)}<span class="text-muted-foreground">offline</span>{/if}
					</span>
				{/each}
				{#if s.workspace}
					<span class="text-muted-foreground"
						>{[s.workspace, s.project].filter(Boolean).join(' › ')}</span
					>
				{/if}
				{#if s.branch}
					<span class="inline-flex items-center gap-0.5 text-muted-foreground"
						><GitBranchIcon class="size-3" />{s.branch}</span
					>
				{/if}
			</div>
			<span class="truncate font-mono text-muted-foreground" title={s.cwd}>{s.cwd}</span>
		</div>

		<!-- what to do with it -->
		<div class="flex flex-col gap-2">
			<div class="flex flex-wrap items-center gap-2">
				{#if s.open}
					<Button size="sm" href="/sessions/{s.id}"><ExternalLinkIcon /> Go to it</Button>
				{:else}
					<Button
						size="sm"
						disabled={Boolean(why) || !live.hosts.some((h) => h.connected)}
						title={why ??
							(live.hosts.some((h) => h.connected) ? 'Resume (Enter)' : 'No machine is connected')}
						onclick={resume}><PlayIcon /> Resume…</Button
					>
					{#if s.known}
						<Button size="sm" variant="outline" href="/sessions/{s.id}"
							><ExternalLinkIcon /> Its page</Button
						>
					{/if}
				{/if}
				<Button
					size="sm"
					variant="ghost"
					title="Copy the command that resumes it in a terminal"
					onclick={() => copy('cmd', resumeCommand(s))}
				>
					{#if copied === 'cmd'}<CheckIcon /> Copied{:else}<CopyIcon /> Terminal command{/if}
				</Button>
			</div>
			{#if why && !s.open}
				<p class="text-xs text-muted-foreground">{why}</p>
			{/if}
		</div>

		<!-- tabs -->
		<div class="flex gap-1 border-b text-sm" role="tablist">
			{#each [['overview', 'Overview'], ['conversation', 'Conversation']] as [value, name] (value)}
				<button
					type="button"
					role="tab"
					aria-selected={tab === value}
					class="-mb-px border-b-2 px-2 py-1.5 {tab === value
						? 'border-primary font-medium text-foreground'
						: 'border-transparent text-muted-foreground hover:text-foreground'}"
					onclick={() =>
						value === 'conversation'
							? openConversation(conv ? { offset: conv.offset } : {})
							: (tab = 'overview')}
					>{name}{#if value === 'conversation'}<span class="ml-1 text-xs text-muted-foreground"
							>{s.prompts + s.replies}</span
						>{/if}</button
				>
			{/each}
		</div>

		{#if tab === 'overview'}
			{#if s.recap}
				<section class="flex flex-col gap-1">
					<h3 class="text-xs font-semibold text-muted-foreground uppercase">Recap</h3>
					<p class="rounded-lg border-l-2 border-primary/60 bg-muted/40 px-3 py-2 text-sm">
						{s.recap}
					</p>
				</section>
			{/if}

			{#if detail.first_prompt}
				<section class="flex flex-col gap-1">
					<h3 class="text-xs font-semibold text-muted-foreground uppercase">
						You asked <span class="font-normal normal-case"
							>· {fmtWhenShort(detail.first_prompt.at, live.now)}</span
						>
					</h3>
					<p class="text-sm whitespace-pre-wrap {fullAsk ? '' : 'line-clamp-[10]'}">
						{detail.first_prompt.text}
					</p>
					{#if !fullAsk && detail.first_prompt.text.split('\n').length > 10}
						<Button variant="ghost" size="xs" class="self-start" onclick={() => (fullAsk = true)}
							>Show all</Button
						>
					{/if}
				</section>
			{/if}

			{#if detail.last_prompt || detail.last_reply_html}
				<section class="flex flex-col gap-2">
					<h3 class="text-xs font-semibold text-muted-foreground uppercase">
						Where it ended
						{#if detail.last_reply_at}<span class="font-normal normal-case"
								>· {fmtWhenShort(detail.last_reply_at, live.now)}</span
							>{/if}
					</h3>
					{#if detail.last_prompt}
						<p class="line-clamp-6 rounded-lg bg-muted/40 px-3 py-2 text-sm whitespace-pre-wrap">
							<span class="font-medium text-muted-foreground">You:</span>
							{detail.last_prompt.text}
						</p>
					{/if}
					{#if detail.last_reply_html}
						<div class="text-sm"><Prose html={detail.last_reply_html} clamp /></div>
					{/if}
				</section>
			{/if}

			{#if detail.matches.length}
				<section class="flex flex-col gap-1">
					<h3 class="text-xs font-semibold text-muted-foreground uppercase">
						Matches <span class="font-normal normal-case">· {detail.matches.length}</span>
					</h3>
					<ul class="divide-y rounded-lg border">
						{#each detail.matches as m (m.text)}
							<li>
								<button
									type="button"
									class="flex w-full flex-col gap-0.5 px-3 py-1.5 text-left text-xs hover:bg-accent/40"
									onclick={() => openConversation({ around: m.text })}
								>
									<span class="text-muted-foreground"
										>{ROLE[m.role] ?? m.role} · {fmtWhenShort(m.at, live.now)}</span
									>
									<span class="line-clamp-2"
										>{#each marks(m.snippet) as part, i (i)}{#if i % 2}<mark
													class="rounded bg-warning/30 px-0.5 text-foreground">{part}</mark
												>{:else}{part}{/if}{/each}</span
									>
								</button>
							</li>
						{/each}
					</ul>
				</section>
			{/if}

			{#if detail.prompts.length > 1}
				<section class="flex flex-col gap-1">
					<h3 class="text-xs font-semibold text-muted-foreground uppercase">
						Every prompt <span class="font-normal normal-case">· {s.prompts}</span>
					</h3>
					<ol class="flex flex-col">
						{#each shownPrompts as p (p.id)}
							<li>
								<button
									type="button"
									class="flex w-full gap-2 rounded px-1 py-1 text-left text-xs hover:bg-accent/40"
									title="Read from here"
									onclick={() => openConversation({ around: p.id })}
								>
									<span class="w-24 shrink-0 text-muted-foreground tabular-nums"
										>{fmtWhenShort(p.at, live.now)}</span
									>
									<span
										class="line-clamp-2 min-w-0 {/^\/\S+$/.test(p.text)
											? 'font-mono text-muted-foreground'
											: ''}">{p.text}</span
									>
								</button>
							</li>
						{/each}
					</ol>
					{#if detail.prompts.length > shownPrompts.length}
						<Button variant="ghost" size="xs" class="self-start" onclick={() => (allPrompts = true)}
							>Show all {detail.prompts.length}</Button
						>
					{:else if detail.more_prompts}
						<p class="text-xs text-muted-foreground">
							…and {detail.more_prompts} more in the conversation.
						</p>
					{/if}
				</section>
			{/if}

			{#if detail.files.length}
				<section class="flex flex-col gap-1">
					<h3 class="text-xs font-semibold text-muted-foreground uppercase">Files it edited</h3>
					<div class="flex flex-wrap gap-1">
						{#each detail.files as [path, n] (path)}
							<span class="rounded bg-muted px-1.5 py-0.5 font-mono text-[11px]" title={path}
								>{path.split('/').slice(-2).join('/')}{#if n > 1}<span
										class="text-muted-foreground"
									>
										×{n}</span
									>{/if}</span
							>
						{/each}
					</div>
				</section>
			{/if}

			<dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
				{#each facts as [k, v] (k)}
					<dt class="text-muted-foreground">{k}</dt>
					<dd class="min-w-0">{v}</dd>
				{/each}
			</dl>
		{:else}
			<section class="flex flex-col gap-3">
				{#if convError}
					<p class="text-sm text-critical" role="alert">{convError}</p>
				{:else if !conv}
					<p class="text-sm text-muted-foreground">Loading…</p>
				{:else}
					<p class="text-xs text-muted-foreground">
						Your prompts and Claude's text, without the tool calls{conv.total > conv.items.length
							? ` · ${conv.offset + 1}–${conv.offset + conv.items.length} of ${conv.total}`
							: ''}.
					</p>
					{#if conv.offset > 0}
						<Button
							variant="outline"
							size="xs"
							class="self-center"
							onclick={() =>
								openConversation({ offset: Math.max(0, conv!.offset - CONVERSATION_PAGE) })}
							>Earlier</Button
						>
					{/if}
					{#each conv.items as item (item.id)}
						<article
							data-text={item.id}
							class="flex flex-col gap-1 rounded-lg {item.role === 'you'
								? 'bg-muted/50 px-3 py-2'
								: 'px-1'} {item.id === target ? 'ring-2 ring-primary/40' : ''}"
						>
							<span class="text-xs text-muted-foreground"
								>{ROLE[item.role] ?? item.role} · {fmtWhenShort(item.at, live.now)}</span
							>
							<div class="text-sm"><Prose html={item.html} clamp={item.role === 'claude'} /></div>
						</article>
					{/each}
					{#if conv.offset + conv.items.length < conv.total}
						<Button
							variant="outline"
							size="xs"
							class="self-center"
							onclick={() => openConversation({ offset: conv!.offset + conv!.items.length })}
							>Later</Button
						>
					{/if}
				{/if}
			</section>
		{/if}
	{/if}
</div>
