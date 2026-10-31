<script lang="ts">
	// One machine: its link, the sessions on it now, Claude's usage there
	// over the range in the bar, its copy of Claude's state, and its share
	// of the workspace sync (what moved, what it cost, what is not in step).
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import { Unauthorized } from '#lib/api/client.js';
	import {
		fetchAnalytics,
		fetchSync,
		fetchWorkspace,
		fetchWorkspaceStats,
		type Activity,
		type Analytics,
		type Bucket,
		type SyncView,
		type WorkspaceStats,
		type WorkspaceView
	} from '#lib/api/index.js';
	import SessionCard from '#lib/components/app/SessionCard.svelte';
	import Columns from '#lib/components/charts/Columns.svelte';
	import Figure from '#lib/components/charts/Figure.svelte';
	import StatTile from '#lib/components/charts/StatTile.svelte';
	import Tiles from '#lib/components/charts/Tiles.svelte';
	import StateChip from '#lib/components/workspace/StateChip.svelte';
	import { bucketEdges, bucketName, columns, type Col } from '#lib/analytics.js';
	import { COLOR, fmtCpu, fmtHoursAxis, fmtTokens, STATES, stateLegend } from '#lib/charts.js';
	import { fmtBytes, fmtDuration, fmtStamp, fmtUsd, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { rangeLabel, reachesNow } from '#lib/range.js';
	import { rangeState } from '#lib/range.svelte.js';
	import { ACTIVITY } from '#lib/session.js';
	import {
		corePct,
		costByHost,
		fmtMs,
		fmtPct,
		fmtRate,
		fmtRtt,
		trafficByHost,
		unitHref
	} from '#lib/workspace.js';

	const POLL_MS = 5000;
	const host = $derived(decodeURIComponent(page.params.host!));

	let analytics = $state<Analytics | null>(null);
	let stats = $state<WorkspaceStats | null>(null);
	let sync = $state<SyncView | null>(null);
	let ws = $state<WorkspaceView | null>(null);
	let error = $state('');

	function failed(e: unknown) {
		if (e instanceof Unauthorized) live.needToken = true;
		else error = (e as Error).message;
	}

	async function loadLive() {
		const [s, w] = await Promise.allSettled([fetchSync(), fetchWorkspace()]);
		if (s.status === 'fulfilled') sync = s.value;
		else failed(s.reason);
		if (w.status === 'fulfilled') ws = w.value;
		// NOTE: a 404 means the workspace sync is off: nothing to show, no error
	}

	async function loadRange(h: string) {
		const params = rangeState.params();
		const tz = -new Date().getTimezoneOffset();
		const [a, st] = await Promise.allSettled([
			fetchAnalytics({ ...params, tz, host: h }),
			fetchWorkspaceStats(params)
		]);
		if (h !== host) return;
		if (a.status === 'fulfilled') analytics = a.value;
		else failed(a.reason);
		if (st.status === 'fulfilled') stats = st.value;
	}

	$effect(() => {
		if (!live.ready) return;
		void host;
		loadLive();
		const timer = setInterval(loadLive, POLL_MS);
		return () => clearInterval(timer);
	});

	$effect(() => {
		if (!live.ready) return;
		const h = host;
		const range = rangeState.current;
		untrack(() => loadRange(h));
		if (!reachesNow(range, rangeState.now())) return;
		const timer = setInterval(() => loadRange(h), 300_000);
		return () => clearInterval(timer);
	});

	const link = $derived(live.hosts.find((x) => x.name === host) ?? null);
	const linkLine = $derived.by(() => {
		if (!link) return 'not connected since the hub started';
		const parts = [
			`${link.connected ? 'connected' : 'offline'} for ${fmtDuration(live.now - link.since)}`
		];
		if (link.connected && link.rtt_ms !== null) parts.push(`${fmtRtt(link.rtt_ms)} round trip`);
		return parts.join(' · ');
	});
	const sessions = $derived(
		live.list
			.filter((s) => s.host === host && s.activity !== 'gone')
			.sort((a, b) => b.started_at - a.started_at)
	);
	const use = $derived(analytics?.hosts.find((x) => x.host === host) ?? null);
	const cols = $derived(analytics ? columns(analytics) : []);
	const syncHost = $derived(sync?.hosts.find((x) => x.host === host) ?? null);
	const wsHost = $derived(ws?.hosts.find((x) => x.host === host) ?? null);
	const traffic = $derived(stats ? (trafficByHost(stats.traffic)[host] ?? null) : null);
	const cost = $derived(stats ? (costByHost(stats.cost)[host] ?? null) : null);
	const spanMs = $derived(stats ? Math.max(1, stats.until - stats.since) : 1);
	const transfers = $derived((stats?.transfers ?? []).filter((t) => t.host === host).slice(0, 8));
	const notInStep = $derived(
		(ws?.units ?? [])
			.filter((u) => u.hosts[host] && u.hosts[host].state !== 'in_step')
			.sort((a, b) => a.unit.localeCompare(b.unit))
	);
	const brush = (from: number, to: number) =>
		rangeState.set({ kind: 'absolute', from, to: Math.min(to, rangeState.now()) });
	/** A bucket's milliseconds in a session state. */
	const ms = (b: Bucket, st: Activity) => (b as unknown as Record<string, number>)[`${st}_ms`] ?? 0;
</script>

<div class="mx-auto flex max-w-6xl flex-col gap-5 p-3 sm:p-4">
	<div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
		<a href="/machines" class="text-sm text-muted-foreground hover:underline">Machines</a>
		<h1 class="flex items-center gap-2 text-lg font-semibold">
			<i
				class="inline-block size-2.5 rounded-full {link?.connected
					? 'bg-your-turn'
					: 'bg-critical'}"
				aria-hidden="true"
			></i>
			{host}
		</h1>
		<span class="text-xs text-muted-foreground">{linkLine}</span>
	</div>
	{#if error}<p class="text-sm text-critical" role="alert">{error}</p>{/if}

	<section class="flex flex-col gap-2">
		<h2 class="text-base font-semibold">Sessions now</h2>
		{#if !sessions.length}
			<p class="text-sm text-muted-foreground">No session on this machine.</p>
		{:else}
			<div class="grid gap-2 md:grid-cols-2">
				{#each sessions as s (s.id)}
					<SessionCard session={s} compact />
				{/each}
			</div>
		{/if}
	</section>

	<section class="flex flex-col gap-2">
		<div class="flex items-baseline gap-2">
			<h2 class="text-base font-semibold">Claude on this machine</h2>
			<span class="text-xs text-muted-foreground">{rangeLabel(rangeState.current)}</span>
			<a
				href="/analytics/overview{page.url.search}"
				class="text-xs text-muted-foreground hover:underline">all of Analytics</a
			>
		</div>
		{#if !analytics}
			<p class="text-sm text-muted-foreground">Loading…</p>
		{:else if !use}
			<p class="text-sm text-muted-foreground">No session ran here in the range.</p>
		{:else}
			<Tiles>
				<StatTile name="Sessions" value={use.sessions} sub={plural(use.prompts, 'prompt')} />
				<StatTile
					name="Working"
					value={fmtDuration(use.working_ms)}
					sub="waited on you {fmtDuration(use.permission_ms + use.your_turn_ms)}"
				/>
				<StatTile
					name="Cost"
					value={fmtUsd(use.cost_usd)}
					sub={use.tokens ? `${fmtTokens(use.tokens)} tokens` : 'API-equivalent'}
				/>
				<StatTile
					name="Lines"
					value="+{use.lines_added} −{use.lines_removed}"
					sub="{plural(use.tools, 'tool call')}{use.tool_failures
						? `, ${use.tool_failures} failed`
						: ''}"
				/>
				<StatTile
					name="CPU"
					value={fmtCpu(use.cpu_secs)}
					sub="peak memory {fmtBytes(use.peak_mem_bytes)}"
				/>
			</Tiles>
			<Figure
				title="Activity"
				sub="Hours per {analytics.bucket_ms < 86_400_000 ? 'hour' : 'day'} by state"
				legend={stateLegend(STATES)}
				empty={!cols.length && 'Nothing in the range.'}
			>
				<Columns
					data={cols}
					series={STATES.map((st) => ({
						key: st,
						label: ACTIVITY[st].label,
						color: COLOR[st],
						value: (d: Col) => ms(d.b, st) / 3_600_000
					}))}
					fmtY={fmtHoursAxis}
					tip={(d) => [
						fmtDuration(d.b.working_ms + d.b.permission_ms + d.b.your_turn_ms),
						bucketName(analytics!, d.b.start),
						...STATES.map((st) => `${ACTIVITY[st].label} ${fmtDuration(ms(d.b, st))}`)
					]}
					onbrush={brush}
					edges={bucketEdges(analytics)}
				/>
			</Figure>
		{/if}
	</section>

	<section class="flex flex-col gap-2">
		<div class="flex items-baseline gap-2">
			<h2 class="text-base font-semibold">Claude state</h2>
			<a href="/state" class="text-xs text-muted-foreground hover:underline">the State page</a>
		</div>
		{#if !sync}
			<p class="text-sm text-muted-foreground">Loading…</p>
		{:else if !syncHost?.status}
			<p class="text-sm text-muted-foreground">This machine does not sync Claude's state.</p>
		{:else}
			{@const s = syncHost.status}
			<p class="text-sm">
				{plural(s.files, 'file')}, {fmtBytes(s.bytes)} of what travels{#if s.pending_up || s.pending_down}
					· {s.pending_up} to send, {s.pending_down} to fetch{:else}
					· in step{/if}{#if s.held}
					· {s.held} held while their session runs elsewhere{/if}{#if s.last_pass}
					· last checked {fmtDuration(live.now - s.last_pass)} ago{/if}{#if !s.watching}
					· scanning instead of watching{/if}
				{#if s.error}<span class="text-critical"> · {s.error}</span>{/if}
			</p>
		{/if}
	</section>

	<section class="flex flex-col gap-2">
		<div class="flex items-baseline gap-2">
			<h2 class="text-base font-semibold">Workspace</h2>
			<a href="/workspace/machines" class="text-xs text-muted-foreground hover:underline"
				>every machine</a
			>
		</div>
		{#if !ws}
			<p class="text-sm text-muted-foreground">The workspace sync is off, or still loading.</p>
		{:else if !wsHost?.status}
			<p class="text-sm text-muted-foreground">This machine does not sync the workspace.</p>
		{:else}
			{@const st = wsHost.status}
			<Tiles>
				<StatTile
					name="Units"
					value="{st.in_step} of {st.units}"
					sub={st.pending ? `in step · ${st.pending} moving` : 'in step'}
				/>
				<StatTile
					name="Moved in the range"
					value={traffic ? fmtBytes(traffic.bytes_in + traffic.bytes_out) : '—'}
					sub={traffic
						? `↑ ${fmtBytes(traffic.bytes_in)} · ↓ ${fmtBytes(traffic.bytes_out)} · ${plural(traffic.requests, 'request')}${traffic.pushes ? ` · a push took ${fmtMs(traffic.push_ms / traffic.pushes)}` : ''}`
						: 'nothing recorded'}
				/>
				<StatTile
					name="Cost in the range"
					value={cost ? `${((cost.cpu_ms + cost.git_cpu_ms) / 1000).toFixed(1)} s CPU` : '—'}
					sub={cost
						? `${fmtPct(corePct(cost.cpu_ms + cost.git_cpu_ms, spanMs))} of one core · ${plural(cost.passes, 'pass', 'passes')} · read ${fmtBytes(cost.io_read)}, wrote ${fmtBytes(cost.io_write)}`
						: 'nothing recorded'}
				/>
				<StatTile
					name="Collector now"
					value={st.cost ? fmtBytes(st.cost.rss_kb * 1024) : '—'}
					sub={st.cost
						? `resident · ${plural(st.cost.watches, 'watch', 'watches')} · ${((st.cost.cpu_ms + st.cost.git_cpu_ms) / 1000).toFixed(0)} s CPU since start`
						: ''}
				/>
			</Tiles>
			{#if st.error}<p class="text-sm text-critical">{st.error}</p>{/if}
			{#if notInStep.length}
				<div class="rounded-lg border bg-card p-3">
					<div class="mb-1 text-sm font-medium">Not in step here</div>
					<ul class="flex flex-col divide-y text-sm">
						{#each notInStep as u (u.unit)}
							{@const s = u.hosts[host]}
							<li class="flex flex-wrap items-center gap-x-3 gap-y-0.5 py-1">
								<a href={unitHref(u.unit)} class="font-mono text-xs hover:underline">{u.unit}</a>
								<StateChip state={s.state} detail={s.detail} />
								{#if s.detail}<span class="text-xs text-muted-foreground">{s.detail}</span>{/if}
							</li>
						{/each}
					</ul>
				</div>
			{:else}
				<p class="text-xs text-muted-foreground">Every unit is in step on this machine.</p>
			{/if}
			{#if transfers.length}
				<div class="rounded-lg border bg-card p-3">
					<div class="mb-1 text-sm font-medium">Large transfers in the range</div>
					<ul class="flex flex-col divide-y text-xs">
						{#each transfers as t (t.at + t.unit)}
							<li class="flex flex-wrap items-center gap-x-3 py-1">
								<span class="text-muted-foreground">{fmtStamp(t.at)}</span>
								<a href={unitHref(t.unit)} class="font-mono hover:underline">{t.unit}</a>
								<span class="text-muted-foreground">{t.kind === 'push' ? 'pushed' : 'fetched'}</span
								>
								<span class="ml-auto tabular-nums"
									>{fmtBytes(t.bytes)} in {fmtMs(t.ms)} · {fmtRate(t.bytes, t.ms)}</span
								>
							</li>
						{/each}
					</ul>
				</div>
			{/if}
		{/if}
	</section>
</div>
