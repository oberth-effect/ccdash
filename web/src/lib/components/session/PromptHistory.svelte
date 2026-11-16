<script lang="ts">
	// The prompt history as a dialog: a search over every prompt in scope
	// (the hub looks, so it reaches past what is loaded), the matched words
	// set apart, each prompt with its session, its time and its length. With
	// a keyboard the arrows move and Enter uses the prompt; on a wide screen
	// the picked one shows in full beside the list, on a phone a tap on the
	// chevron opens it in place.
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import SearchIcon from '@lucide/svelte/icons/search';
	import { Unauthorized } from '#lib/api/client.js';
	import type { Prompt } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { fmtDate, fmtTime, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { label } from '#lib/session.js';

	const SEARCH_DELAY_MS = 250;
	/** Characters shown before a match in a row's excerpt. */
	const LEAD = 60;
	const DAY_MS = 86_400_000;

	let {
		open = $bindable(false),
		recent,
		search,
		session,
		note,
		onuse,
		oninsert
	}: {
		open?: boolean;
		/** The loaded history, newest first. */
		recent: Prompt[];
		/** Asks the hub for the prompts that hold every word. */
		search: (query: string) => Promise<Prompt[]>;
		/** The session the field writes to: its prompts can be shown alone. */
		session?: string;
		/** What the history covers, one line. */
		note: string;
		/** The prompt replaces the field's text. */
		onuse: (p: Prompt) => void;
		/** The prompt goes in at the cursor. */
		oninsert: (p: Prompt) => void;
	} = $props();

	let query = $state('');
	let own = $state(false);
	let found = $state<Prompt[] | null>(null);
	let searching = $state(false);
	/** The hub did not answer the search: only what is loaded was looked through. */
	let partial = $state(false);
	let pick = $state(0);
	let opened = $state<string | null>(null);
	let copied = $state(false);
	let list = $state<HTMLElement | null>(null);

	const words = $derived(query.toLowerCase().split(/\s+/).filter(Boolean));
	const holds = (p: Prompt) => {
		const text = p.text.toLowerCase();
		return words.every((w) => text.includes(w));
	};
	/** NOTE: the field keeps its own session first, for Ctrl ↑; here time orders. */
	const byTime = $derived([...recent].sort((a, b) => b.at - a.at));
	/** What is loaded answers at once; the hub's answer replaces it. */
	const rows = $derived(
		(words.length ? (found ?? recent.filter(holds)) : byTime).filter(
			(p) => !own || p.session === session
		)
	);
	const current = $derived<Prompt | undefined>(rows[Math.min(pick, rows.length - 1)]);
	const key = (p: Prompt) => `${p.at} ${p.session}`;

	$effect(() => {
		const q = query.trim();
		found = null;
		partial = false;
		pick = 0;
		if (!q) {
			searching = false;
			return;
		}
		searching = true;
		let stale = false;
		const timer = setTimeout(async () => {
			try {
				const prompts = await search(q);
				if (!stale) found = prompts;
			} catch (e) {
				if (e instanceof Unauthorized) live.needToken = true;
				else if (!stale) partial = true;
			} finally {
				if (!stale) searching = false;
			}
		}, SEARCH_DELAY_MS);
		return () => {
			stale = true;
			clearTimeout(timer);
		};
	});

	$effect(() => {
		if (open) return;
		query = '';
		opened = null;
		copied = false;
	});

	/** The text cut into plain and matched pieces. */
	function pieces(text: string): { text: string; hit: boolean }[] {
		if (!words.length) return [{ text, hit: false }];
		const pattern = new RegExp(
			`(${words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|')})`,
			'gi'
		);
		return text
			.split(pattern)
			.filter(Boolean)
			.map((part, i) => ({ text: part, hit: i % 2 === 1 }));
	}

	/** A row's lines: from just before the first match when searching. */
	function excerpt(text: string): string {
		const body = text.trim();
		if (!words.length) return body;
		const lower = body.toLowerCase();
		const first = Math.min(...words.map((w) => lower.indexOf(w)).filter((i) => i >= 0));
		if (!Number.isFinite(first) || first <= LEAD) return body;
		const from = body.lastIndexOf(' ', first - LEAD / 2);
		return `…${body.slice(from < 0 ? first - LEAD / 2 : from + 1)}`;
	}

	function size(text: string): string {
		const lines = text.trim().split('\n').length;
		const chars = text.length >= 1000 ? `${(text.length / 1000).toFixed(1)}k` : `${text.length}`;
		return lines > 1 ? `${plural(lines, 'line')} · ${chars} chars` : `${chars} chars`;
	}

	function day(at: number): string {
		const date = fmtDate(at);
		if (date === fmtDate(live.now)) return 'Today';
		if (date === fmtDate(live.now - DAY_MS)) return 'Yesterday';
		return date;
	}

	function from(p: Prompt): string {
		if (session && p.session === session) return 'This session';
		const known = live.sessions.get(p.session);
		return p.title ?? (known ? label(known) : `Session ${p.session.slice(0, 8)}`);
	}

	function move(step: number) {
		if (!rows.length) return;
		pick = Math.min(rows.length - 1, Math.max(0, pick + step));
		requestAnimationFrame(() =>
			list?.querySelector('[aria-selected=true]')?.scrollIntoView({ block: 'nearest' })
		);
	}

	function onkeydown(event: KeyboardEvent) {
		if (event.key === 'ArrowDown') move(1);
		else if (event.key === 'ArrowUp') move(-1);
		else if (event.key === 'PageDown') move(8);
		else if (event.key === 'PageUp') move(-8);
		else if (event.key === 'Enter' && current) {
			if (event.shiftKey) oninsert(current);
			else onuse(current);
		} else return;
		event.preventDefault();
	}

	async function copy(p: Prompt) {
		await navigator.clipboard.writeText(p.text);
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}
</script>

{#snippet marked(text: string)}
	{#each pieces(text) as piece, i (i)}{#if piece.hit}<mark
				class="rounded-sm bg-warning/35 text-foreground">{piece.text}</mark
			>{:else}{piece.text}{/if}{/each}
{/snippet}

<Dialog.Root bind:open>
	<Dialog.Content
		class="flex h-[min(80dvh,46rem)] w-[min(96vw,64rem)] max-w-none flex-col gap-0 overflow-hidden p-0 max-sm:h-dvh max-sm:w-screen max-sm:max-w-none max-sm:rounded-none sm:max-w-none"
		{onkeydown}
	>
		<Dialog.Header class="gap-0.5 border-b px-4 pt-3 pb-2.5">
			<Dialog.Title>Prompt history</Dialog.Title>
			<Dialog.Description class="text-xs">{note}</Dialog.Description>
		</Dialog.Header>
		<div class="flex flex-wrap items-center gap-2 border-b px-4 py-2">
			<label class="relative min-w-40 flex-1">
				<SearchIcon
					class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
				/>
				<!-- svelte-ignore a11y_autofocus -->
				<input
					bind:value={query}
					type="search"
					autofocus
					placeholder="Search every prompt: all the words, any order"
					aria-label="Search prompts"
					autocomplete="off"
					spellcheck={false}
					class="h-8 w-full rounded-lg border border-input bg-transparent pr-2 pl-8 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
				/>
			</label>
			{#if session}
				<div class="flex gap-1" role="group" aria-label="Which prompts">
					<Button
						variant={own ? 'ghost' : 'secondary'}
						size="sm"
						aria-pressed={!own}
						onclick={() => (own = false)}>All</Button
					>
					<Button
						variant={own ? 'secondary' : 'ghost'}
						size="sm"
						aria-pressed={own}
						onclick={() => (own = true)}>This session</Button
					>
				</div>
			{/if}
			<span class="text-xs text-muted-foreground tabular-nums" role="status">
				{searching ? 'Searching…' : plural(rows.length, 'prompt')}
			</span>
			{#if partial}
				<p class="w-full text-xs text-warning" role="alert">
					The hub did not answer: only the prompts already loaded were searched.
				</p>
			{/if}
		</div>
		<div class="flex min-h-0 flex-1">
			<ul
				bind:this={list}
				role="listbox"
				aria-label="Prompts"
				class="min-h-0 w-full overflow-y-auto md:w-[26rem] md:shrink-0 md:border-r"
			>
				{#each rows as p, i (key(p))}
					{@const head = !words.length && (i === 0 || day(rows[i - 1].at) !== day(p.at))}
					{@const full = opened === key(p)}
					{#if head}
						<li
							role="presentation"
							class="sticky top-0 z-10 border-b bg-popover/95 px-4 py-1 text-xs font-medium text-muted-foreground backdrop-blur"
						>
							{day(p.at)}
						</li>
					{/if}
					<li
						role="option"
						aria-selected={i === pick}
						class="flex items-start border-b border-border/60 {i === pick ? 'bg-accent/70' : ''}"
					>
						<button
							type="button"
							tabindex="-1"
							class="flex min-w-0 flex-1 flex-col gap-1 py-2 pr-1 pl-4 text-left md:pr-4"
							onpointermove={() => (pick = i)}
							onclick={() => onuse(p)}
						>
							<span class="flex w-full items-baseline gap-2 text-xs text-muted-foreground">
								<span
									class="truncate {session && p.session === session
										? ''
										: 'font-medium text-foreground/80'}">{from(p)}</span
								>
								<span class="ml-auto shrink-0 tabular-nums"
									>{words.length ? `${fmtDate(p.at)} ` : ''}{fmtTime(p.at)}</span
								>
							</span>
							<span
								class="w-full text-sm break-words whitespace-pre-wrap {full ? '' : 'line-clamp-3'}"
								>{@render marked(full ? p.text.trim() : excerpt(p.text))}</span
							>
							<span class="text-xs text-muted-foreground tabular-nums">{size(p.text)}</span>
						</button>
						<Button
							variant="ghost"
							size="icon-sm"
							class="mt-1.5 mr-2 shrink-0 md:hidden"
							aria-label={full ? 'Show less of the prompt' : 'Show the whole prompt'}
							aria-expanded={full}
							onclick={() => (opened = full ? null : key(p))}
						>
							<ChevronDownIcon class="transition-transform {full ? 'rotate-180' : ''}" />
						</Button>
					</li>
				{:else}
					<li class="px-4 py-6 text-center text-sm text-muted-foreground">
						{#if searching}
							Searching…
						{:else if words.length}
							No prompt holds {words.length > 1 ? 'all of these words' : 'this word'}.
						{:else}
							No prompts yet.
						{/if}
					</li>
				{/each}
			</ul>
			<div class="hidden min-w-0 flex-1 flex-col md:flex">
				{#if current}
					<div class="min-h-0 flex-1 overflow-y-auto px-4 py-3">
						<p class="mb-2 text-xs text-muted-foreground">
							<span class="font-medium text-foreground/80">{from(current)}</span>
							· {fmtDate(current.at)}
							{fmtTime(current.at)} · {size(current.text)}
						</p>
						<p class="text-sm break-words whitespace-pre-wrap">
							{@render marked(current.text.trim())}
						</p>
					</div>
					<div class="flex flex-wrap items-center gap-2 border-t px-4 py-2">
						<span class="mr-auto text-xs text-muted-foreground">
							↑ ↓ to move · Enter to use · Shift Enter to insert
						</span>
						<Button variant="ghost" size="sm" onclick={() => copy(current)}>
							<CopyIcon />
							{copied ? 'Copied' : 'Copy'}
						</Button>
						<Button variant="outline" size="sm" onclick={() => oninsert(current)}
							>Insert at the cursor</Button
						>
						<Button size="sm" onclick={() => onuse(current)}>Use</Button>
					</div>
				{/if}
			</div>
		</div>
	</Dialog.Content>
</Dialog.Root>
