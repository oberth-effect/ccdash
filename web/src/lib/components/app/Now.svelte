<script lang="ts">
	// The page left open on a second monitor. On the left what needs the
	// owner, with the answer inline and the session's actions (Resolved and
	// Kill pinned), then what is working, each session told well enough not
	// to open it, then the rest that is open. On the right the limits over
	// their windows, the system and the last progress; a phone stacks them.
	// "Start a session" opens the start sheet in the scope's workspace.
	import PlayIcon from '@lucide/svelte/icons/play';
	import { Unauthorized } from '#lib/api/client.js';
	import {
		fetchHistory,
		fetchLimitTracks,
		fetchProjects,
		type Entry,
		type Lane,
		type LimitTrack,
		type Projects
	} from '#lib/api/index.js';
	import { fmtDuration, plain, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { scope } from '#lib/scope.svelte.js';
	import { busy, group, label, shown } from '#lib/session.js';
	import { starter } from '#lib/starter.svelte.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import AskCard from './AskCard.svelte';
	import Clock from './Clock.svelte';
	import LimitTile from './LimitTile.svelte';
	import NowProgress from './NowProgress.svelte';
	import NowSystem from './NowSystem.svelte';
	import QuickReply from './QuickReply.svelte';
	import SessionActions from './SessionActions.svelte';
	import SessionGlance from './SessionGlance.svelte';
	import StateBadge from './StateBadge.svelte';

	const UNDELIVERED_MS = 20_000;
	const STRIP_MS = 60 * 60_000;
	const PROGRESS = 12;
	const OTHERS = 8;
	const LIMITS_EVERY_MS = 60_000;

	const groups = $derived(group(live.list.filter((s) => scope.covers(s))));
	const holding = $derived(
		groups.all.filter(
			(s) =>
				s.activity !== 'gone' &&
				!groups.needs.includes(s) &&
				((s.outbox?.length &&
					!busy(s) &&
					live.now - s.outbox[s.outbox.length - 1].at > UNDELIVERED_MS) ||
					s.inbox?.length)
		)
	);
	const longest = $derived(
		groups.needs.length
			? live.now - groups.needs.reduce((m, s) => Math.min(m, s.activity_since), Infinity)
			: 0
	);
	const hostCounts = $derived.by(() => {
		const counts: Record<string, number> = {};
		for (const s of groups.all)
			if (s.activity !== 'gone') counts[s.host] = (counts[s.host] ?? 0) + 1;
		return counts;
	});
	const others = $derived([...groups.agents, ...groups.parked, ...groups.idle]);
	const cols = 'grid gap-3 grid-cols-[repeat(auto-fit,minmax(min(100%,24rem),1fr))]';

	let projects = $state<Projects | null>(null);
	let projectsTimer: ReturnType<typeof setTimeout> | null = null;
	let lanes = $state(new Map<string, Lane>());
	let tracks = $state<LimitTrack[]>([]);
	let tracksTimer: ReturnType<typeof setTimeout> | null = null;

	const entries = $derived(
		(projects?.workspaces ?? [])
			.flatMap((w) => w.projects)
			.flatMap((p): Entry[] => p.progress)
			.sort((a, b) => b.logged_at - a.logged_at)
			.slice(0, PROGRESS)
	);
	const issues = $derived.by(() => {
		if (!projects) return null;
		const open = projects.workspaces.flatMap((w) => w.projects).flatMap((p) => p.issues);
		return {
			open: open.length,
			working: open.filter((i) => i.in_progress).length,
			flagged: projects.workspaces.reduce((n, w) => n + w.flagged.length, 0)
		};
	});
	const track = (key: string) => tracks.find((t) => t.key === key);

	function caught(e: unknown) {
		if (e instanceof Unauthorized) live.needToken = true;
	}

	async function loadProjects() {
		try {
			projects = await fetchProjects({ workspace: scope.params().workspace });
		} catch (e) {
			caught(e);
		}
	}

	async function loadLanes() {
		try {
			const data = await fetchHistory({ since: Math.round(live.now - STRIP_MS) });
			lanes = new Map(data.lanes.map((lane) => [lane.id, lane]));
		} catch (e) {
			caught(e);
		}
	}

	async function loadTracks() {
		try {
			tracks = (await fetchLimitTracks()).tracks;
		} catch (e) {
			caught(e);
		}
	}

	$effect(() => {
		void live.notes.n;
		void scope.current.ws;
		if (!live.ready) return;
		if (projectsTimer) clearTimeout(projectsTimer);
		projectsTimer = setTimeout(loadProjects, 300);
	});

	$effect(() => {
		if (!live.ready) return;
		loadLanes();
		const timer = setInterval(loadLanes, 60_000);
		return () => clearInterval(timer);
	});

	// NOTE: a new reading moves the line's end by itself; this picks up its
	// history and pace, at most once in a while
	$effect(() => {
		void live.rateLimits?.updated_at;
		if (!live.ready || tracksTimer) return;
		tracksTimer = setTimeout(
			() => {
				tracksTimer = null;
				loadTracks();
			},
			tracks.length ? LIMITS_EVERY_MS : 0
		);
	});

	$effect(() => () => {
		if (projectsTimer) clearTimeout(projectsTimer);
		if (tracksTimer) clearTimeout(tracksTimer);
	});
</script>

<div
	class="mx-auto grid max-w-[120rem] grid-cols-1 items-start gap-5 p-3 sm:p-4 xl:grid-cols-[minmax(0,1fr)_26rem]"
>
	<div class="flex min-w-0 flex-col gap-5">
		<section class="flex flex-col gap-3">
			<div class="flex flex-wrap items-center gap-2">
				<h2 class="flex items-baseline gap-2 text-base font-semibold">
					Needs you
					<span class="text-sm font-normal text-muted-foreground">
						{#if groups.needs.length}{plural(groups.needs.length, 'session')} · longest wait {fmtDuration(
								longest
							)}{:else if live.ready}nothing is waiting on you{/if}
					</span>
				</h2>
				<Button
					size="sm"
					class="ml-auto"
					disabled={!live.hosts.some((h) => h.connected)}
					title={live.hosts.some((h) => h.connected) ? undefined : 'No host is connected'}
					onclick={() => starter.show({ ws: scope.current.ws[0] ?? null })}
				>
					<PlayIcon /> Start a session
				</Button>
			</div>
			{#if groups.needs.length}
				<div class={cols}>
					{#each groups.needs as s (s.id)}
						<SessionGlance session={s} lane={lanes.get(s.id)}>
							{#if s.asks.length}
								<AskCard session={s.id} ask={s.asks[0]} count={s.asks.length} />
							{:else if s.hooked && s.kind !== 'background'}
								<QuickReply session={s} />
							{/if}
							<div class="flex justify-end">
								<SessionActions session={s} pin={['resolve', 'kill']} />
							</div>
						</SessionGlance>
					{/each}
				</div>
			{/if}
		</section>

		{#if holding.length}
			<section class="flex flex-col gap-2">
				<h2 class="text-base font-semibold">Holding messages</h2>
				{#each holding as s (s.id)}
					<a
						href="/sessions/{s.id}"
						class="flex flex-col gap-1 rounded-lg border bg-card p-3 text-sm"
					>
						<span class="flex items-center gap-2">
							<StateBadge activity={shown(s)} />
							<span class="truncate font-medium">{label(s)}</span>
						</span>
						{#if s.outbox?.length}
							<span class="text-xs text-warning"
								>Reply not picked up yet: check the terminal · {plain(
									s.outbox[s.outbox.length - 1].text
								)}</span
							>
						{/if}
						{#if s.inbox?.length}
							<span class="text-xs text-muted-foreground"
								>{plural(s.inbox.length, 'message')} waiting for its next prompt</span
							>
						{/if}
					</a>
				{/each}
			</section>
		{/if}

		<section class="flex flex-col gap-2">
			<h2 class="flex items-baseline gap-2 text-base font-semibold">
				Working <span class="text-sm font-normal text-muted-foreground"
					>{groups.working.length}</span
				>
			</h2>
			{#if groups.working.length}
				<div class={cols}>
					{#each groups.working as s (s.id)}
						<SessionGlance session={s} lane={lanes.get(s.id)} />
					{/each}
				</div>
			{:else if live.ready}
				<p class="text-sm text-muted-foreground">Nothing is working.</p>
			{/if}
		</section>

		{#if others.length}
			<section class="flex flex-col gap-2">
				<h2 class="flex items-baseline gap-2 text-base font-semibold">
					Also open <span class="text-sm font-normal text-muted-foreground">{others.length}</span>
				</h2>
				<ul class="divide-y rounded-lg border bg-card text-sm">
					{#each others.slice(0, OTHERS) as s (s.id)}
						<li>
							<a
								href="/sessions/{s.id}"
								class="flex min-w-0 items-center gap-2 px-3 py-1.5 hover:bg-accent/40"
							>
								<StateBadge activity={shown(s)} class="shrink-0" />
								<span class="min-w-0 truncate">{label(s)}</span>
								<Clock since={s.activity_since} class="ml-auto shrink-0 text-xs" />
							</a>
						</li>
					{/each}
					{#if others.length > OTHERS}
						<li>
							<a
								href="/sessions"
								class="block px-3 py-1.5 text-xs text-muted-foreground hover:underline"
								>{others.length - OTHERS} more on Sessions</a
							>
						</li>
					{/if}
				</ul>
			</section>
		{/if}
	</div>

	<aside class="flex min-w-0 flex-col gap-5">
		<section class="flex flex-col gap-2">
			<h2 class="text-base font-semibold">Limits</h2>
			<div class="grid grid-cols-2 gap-2">
				<LimitTile
					name="5-hour limit"
					limit={live.rateLimits?.five_hour}
					track={track('five_hour')}
				/>
				<LimitTile
					name="7-day limit"
					limit={live.rateLimits?.seven_day}
					track={track('seven_day')}
				/>
				{#each live.rateLimits?.scoped ?? [] as l (l.label)}
					<LimitTile name="{l.label} limit" limit={l} track={track(l.label)} />
				{/each}
			</div>
		</section>

		<section class="flex flex-col gap-2">
			<h2 class="text-base font-semibold">System</h2>
			<NowSystem {hostCounts} {issues} />
		</section>

		{#if entries.length}
			<section class="flex flex-col gap-2">
				<h2 class="flex items-baseline justify-between gap-2 text-base font-semibold">
					Recent progress
					<a href="/projects" class="text-xs font-normal text-muted-foreground hover:underline"
						>Projects</a
					>
				</h2>
				<NowProgress {entries} />
			</section>
		{/if}
	</aside>
</div>
