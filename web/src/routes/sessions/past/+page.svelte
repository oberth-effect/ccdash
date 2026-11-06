<script lang="ts">
	// Sessions › Past: every conversation the hub holds, from every machine
	// that syncs its Claude Code state, to find one and resume it. A search
	// over what was said (prompts, Claude's replies, titles), the machines
	// (any number), the project, how recent, the order; cards that tell a
	// session apart without opening it; its overview beside the list (its
	// own view on a phone) with Resume on the machine you pick. Filters and
	// the open session live in the URL.
	import { onMount, tick, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import LoaderCircleIcon from '@lucide/svelte/icons/loader-circle';
	import SearchIcon from '@lucide/svelte/icons/search';
	import { Unauthorized } from '#lib/api/client.js';
	import { fetchPast, type PastPage } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import PastCard from '#lib/components/past/PastCard.svelte';
	import PastDetail from '#lib/components/past/PastDetail.svelte';
	import PastFilters from '#lib/components/past/PastFilters.svelte';
	import SessionsTabs from '#lib/components/past/SessionsTabs.svelte';
	import { plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { dayKey, dayName } from '#lib/notes.js';
	import { pastHref, readFilters, sinceOf, sortOf, type Filters } from '#lib/past.js';
	import { scope } from '#lib/scope.svelte.js';
	import { starter } from '#lib/starter.svelte.js';

	const PAGE = 50;
	const TYPE_MS = 250;
	const INDEXING_POLL_MS = 4000;

	const filters = $derived(readFilters(page.url.searchParams));
	const selected = $derived(page.url.searchParams.get('id'));
	const workspace = $derived(scope.params().workspace);
	// NOTE: a string, so the open session changing (`id`) does not fetch the list again
	const listKey = $derived(JSON.stringify([filters, workspace]));

	let text = $state(untrack(() => readFilters(page.url.searchParams).q));
	let result = $state<PastPage | null>(null);
	let loading = $state(false);
	let more = $state(false);
	let error = $state('');
	let wide = $state(true);
	let searchEl = $state<HTMLInputElement | null>(null);
	let detail = $state<PastDetail | null>(null);
	/** The phone's list scroll, kept while a session is open over it. */
	let listScroll = 0;
	let pushed = false;
	let asked = 0;

	const hosts = $derived(
		[
			...new Set([
				...(result?.facets.hosts ?? []).map((h) => h.host),
				...live.hosts.map((h) => h.name)
			])
		].sort()
	);
	const sort = $derived(sortOf(filters));
	const grouped = $derived.by(() => {
		const sessions = result?.sessions ?? [];
		if (sort !== 'recent' && sort !== 'started') return [{ key: '', day: '', items: sessions }];
		const out: { key: string; day: string; items: typeof sessions }[] = [];
		for (const s of sessions) {
			const at = sort === 'started' ? s.first_at : s.last_at;
			const key = dayKey(at);
			const last = out.at(-1);
			if (last?.key === key) last.items.push(s);
			else out.push({ key, day: dayName(at, live.now), items: [s] });
		}
		return out;
	});
	const indexing = $derived(Boolean(result && (!result.ready || result.pending > 20)));
	const filtered = $derived(
		Boolean(
			filters.q.trim() || filters.hosts.length || filters.project || filters.when || workspace
		)
	);

	onMount(() => {
		const mq = matchMedia('(min-width: 1024px)');
		wide = mq.matches;
		const onchange = () => (wide = mq.matches);
		mq.addEventListener('change', onchange);
		// NOTE: the bar's host, when one is picked, is where to look first
		if (!page.url.searchParams.has('host') && scope.current.host)
			set({ hosts: [scope.current.host] });
		if (wide && !selected) searchEl?.focus();
		return () => mq.removeEventListener('change', onchange);
	});

	function set(changes: Partial<Filters>) {
		goto(pastHref({ ...filters, ...changes }, selected), { replace: true, reset: false });
	}

	// The search box writes the URL once typing pauses; back and forward write the box.
	$effect(() => {
		const typed = text;
		const timer = setTimeout(
			() => untrack(() => typed.trim() !== filters.q && set({ q: typed })),
			TYPE_MS
		);
		return () => clearTimeout(timer);
	});
	$effect(() => {
		const q = filters.q;
		untrack(() => {
			if (q !== text.trim()) text = q;
		});
	});

	async function load(offset: number) {
		const f = readFilters(page.url.searchParams);
		const n = ++asked;
		if (offset) more = true;
		else loading = true;
		try {
			const got = await fetchPast({
				q: f.q.trim() || undefined,
				host: f.hosts.join(',') || undefined,
				workspace: scope.params().workspace,
				project: f.project || undefined,
				since: sinceOf(f.when, Date.now()),
				sort: sortOf(f),
				offset,
				limit: PAGE
			});
			if (n !== asked) return;
			result = offset && result ? { ...got, sessions: [...result.sessions, ...got.sessions] } : got;
			error = '';
		} catch (e) {
			if (n !== asked) return;
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		} finally {
			if (n === asked) loading = more = false;
		}
	}

	$effect(() => {
		void listKey;
		if (!live.ready) return;
		untrack(() => load(0));
	});

	// While the hub reads transcripts the list grows: ask again now and then.
	$effect(() => {
		if (!indexing) return;
		const timer = setInterval(() => untrack(() => load(0)), INDEXING_POLL_MS);
		return () => clearInterval(timer);
	});

	function open(id: string) {
		if (id === selected) return;
		if (!wide) {
			listScroll = document.querySelector('main')?.scrollTop ?? 0;
			pushed = true;
		}
		goto(pastHref(filters, id), { replace: wide, reset: false });
	}

	function close() {
		if (!wide && pushed) {
			pushed = false;
			history.back();
		} else {
			goto(pastHref(filters, null), { replace: true, reset: false });
		}
	}

	// Back on the phone (the button, the system's back): the list where it was.
	let shownBefore: string | null = null;
	$effect(() => {
		const now = selected;
		if (shownBefore && !now && !wide) {
			const y = listScroll;
			tick().then(() => document.querySelector('main')?.scrollTo(0, y));
		}
		shownBefore = now;
	});

	function select(event: MouseEvent, id: string) {
		if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
		event.preventDefault();
		open(id);
	}

	function move(by: number) {
		const list = result?.sessions ?? [];
		if (!list.length) return;
		const at = list.findIndex((s) => s.id === selected);
		const next = list[Math.min(list.length - 1, Math.max(0, at < 0 ? 0 : at + by))];
		open(next.id);
		tick().then(() =>
			document.querySelector(`[data-session="${next.id}"]`)?.scrollIntoView({ block: 'nearest' })
		);
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey || starter.open) return;
		const el = e.target as HTMLElement;
		if (el.closest('[role="menu"], [role="dialog"], [role="listbox"]')) return;
		const typing = Boolean(el.closest('input, textarea, select, [contenteditable="true"]'));
		const inSearch = el === searchEl;
		if (typing && !inSearch) return;
		if (e.key === '/' && !typing) {
			e.preventDefault();
			searchEl?.focus();
			searchEl?.select();
		} else if (e.key === 'ArrowDown' || (!typing && e.key === 'j')) {
			e.preventDefault();
			move(1);
		} else if (e.key === 'ArrowUp' || (!typing && e.key === 'k')) {
			e.preventDefault();
			move(-1);
		} else if (e.key === 'Enter') {
			if (inSearch && text.trim() !== filters.q) set({ q: text });
			else if (selected) detail?.resumeNow();
			else if (result?.sessions[0]) open(result.sessions[0].id);
			else return;
			e.preventDefault();
		} else if (e.key === 'Escape') {
			if (selected) close();
			else if (inSearch) searchEl?.blur();
		}
	}
</script>

<svelte:head>
	<title>Past sessions · ccdash</title>
</svelte:head>

<svelte:window {onkeydown} />

<div class="lg:grid lg:h-full lg:grid-cols-[minmax(0,1fr)_minmax(26rem,42%)]">
	<section
		class="flex min-w-0 flex-col gap-3 p-3 sm:p-4 lg:overflow-y-auto {selected && !wide
			? 'hidden'
			: ''}"
		aria-label="Past sessions"
	>
		<SessionsTabs />

		<label class="relative block">
			<span class="sr-only">Search past sessions</span>
			<SearchIcon
				class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
			/>
			<input
				bind:this={searchEl}
				bind:value={text}
				type="search"
				enterkeyhint="search"
				autocomplete="off"
				placeholder="Search what was said: your prompts, Claude's replies, titles"
				class="h-10 w-full rounded-lg border border-input bg-transparent pr-10 pl-9 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 dark:bg-input/30"
			/>
			{#if loading}
				<LoaderCircleIcon
					class="absolute top-1/2 right-3 size-4 -translate-y-1/2 animate-spin text-muted-foreground"
				/>
			{:else if !text}
				<kbd
					class="absolute top-1/2 right-3 hidden -translate-y-1/2 rounded border px-1.5 text-[11px] text-muted-foreground sm:block"
					>/</kbd
				>
			{/if}
		</label>

		<PastFilters {filters} facets={result?.facets ?? null} {hosts} onchange={set} />

		{#if indexing && result}
			<p
				class="rounded-lg border border-dashed px-3 py-2 text-xs text-muted-foreground"
				role="status"
			>
				{#if !result.ready}
					The hub is reading the transcripts for the first time{result.pending
						? `: ${result.pending} to go`
						: ''}. The list fills in as it goes.
				{:else}
					Reading {result.pending} new transcripts; the list fills in as it goes.
				{/if}
			</p>
		{/if}

		{#if error}
			<p class="text-sm text-critical" role="alert">{error}</p>
		{:else if result}
			<p class="text-xs text-muted-foreground" role="status">
				{#if result.matched === 'any'}
					No session has all the words; {plural(result.total, 'session has', 'sessions have')} some of
					them.
				{:else if result.matched === 'all'}
					{filters.q.trim().split(/\s+/).length > 1
						? `${plural(result.total, 'session')} with every word`
						: `${plural(result.total, 'session mentions', 'sessions mention')} “${filters.q.trim()}”`}
				{:else}
					{plural(result.total, 'session')}{filtered && result.total < result.indexed
						? ` of ${result.indexed}`
						: ''}
				{/if}
			</p>
		{/if}

		{#if result && !result.sessions.length && !loading}
			<div class="flex flex-col items-start gap-2 rounded-lg border border-dashed p-4 text-sm">
				{#if !result.indexed}
					<p class="text-muted-foreground">
						No transcripts on the hub yet. A machine whose ccdash collector has the state sync on
						sends its sessions; they show here as the hub reads them.
					</p>
				{:else}
					<p class="text-muted-foreground">No session matches.</p>
					<div class="flex flex-wrap gap-2">
						{#if filters.q}
							<Button variant="outline" size="sm" onclick={() => (text = '')}
								>Clear the search</Button
							>
						{/if}
						{#if filters.hosts.length || filters.project || filters.when}
							<Button
								variant="outline"
								size="sm"
								onclick={() => set({ hosts: [], project: '', when: '' })}
								>All machines, projects and times</Button
							>
						{/if}
						{#if workspace}
							<Button variant="outline" size="sm" onclick={() => scope.set({ ws: [] })}
								>Every workspace</Button
							>
						{/if}
					</div>
				{/if}
			</div>
		{/if}

		{#each grouped as g (g.key)}
			<section class="flex flex-col gap-2">
				{#if g.day}
					<h2
						class="sticky top-0 z-10 -mx-1 bg-background/90 px-1 py-0.5 text-xs font-semibold text-muted-foreground backdrop-blur"
					>
						{g.day} <span class="font-normal">{g.items.length}</span>
					</h2>
				{/if}
				{#each g.items as s (s.id)}
					<PastCard
						session={s}
						{hosts}
						href={pastHref(filters, s.id)}
						selected={s.id === selected}
						now={live.now}
						onselect={(e) => select(e, s.id)}
					/>
				{/each}
			</section>
		{/each}

		{#if result && result.sessions.length < result.total}
			<Button
				variant="outline"
				size="sm"
				class="self-center"
				disabled={more}
				onclick={() => load(result!.sessions.length)}
			>
				{#if more}<LoaderCircleIcon class="animate-spin" />{/if}
				Show more ({result.total - result.sessions.length} left)
			</Button>
		{/if}
	</section>

	<aside
		class="min-w-0 lg:overflow-y-auto lg:border-l {selected || wide ? '' : 'hidden'}"
		aria-label="The session"
	>
		{#if selected}
			{#key selected}
				<PastDetail bind:this={detail} id={selected} q={filters.q} {hosts} {wide} onclose={close} />
			{/key}
		{:else}
			<div
				class="flex h-full flex-col items-center justify-center gap-3 p-8 text-center text-sm text-muted-foreground"
			>
				<p class="max-w-xs">
					Pick a session to see what it was about, where it ran, and resume it on the machine you
					choose.
				</p>
				<p class="flex flex-wrap justify-center gap-x-3 gap-y-1 text-xs">
					<span><kbd class="rounded border px-1">/</kbd> search</span>
					<span
						><kbd class="rounded border px-1">↑</kbd> <kbd class="rounded border px-1">↓</kbd> move</span
					>
					<span><kbd class="rounded border px-1">Enter</kbd> resume</span>
					<span><kbd class="rounded border px-1">Esc</kbd> close</span>
				</p>
			</div>
		{/if}
	</aside>
</div>
