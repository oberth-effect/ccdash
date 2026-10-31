<script lang="ts">
	// Workspace › Overview: the tiles, what needs you, the machines in a
	// line each, the last day's traffic and the units that changed last.
	import { Unauthorized } from '#lib/api/client.js';
	import { fetchWorkspaceStats, type WorkspaceStats, type WorkspaceView } from '#lib/api/index.js';
	import Figure from '#lib/components/charts/Figure.svelte';
	import StatTile from '#lib/components/charts/StatTile.svelte';
	import Tiles from '#lib/components/charts/Tiles.svelte';
	import { fmtBytes, fmtStamp, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import {
		corePct,
		costByHost,
		fmtPct,
		fmtRtt,
		hostColor,
		trafficByHost,
		unitHref
	} from '#lib/workspace.js';
	import HeldList from './HeldList.svelte';
	import HostsStrip from './HostsStrip.svelte';
	import NewDirs from './NewDirs.svelte';
	import TrafficChart from './TrafficChart.svelte';
	import WsConflicts from './WsConflicts.svelte';

	let { view, onchange }: { view: WorkspaceView; onchange: () => void } = $props();

	const STATS_EVERY_MS = 60_000;
	const DAY_MS = 86_400_000;

	let stats = $state<WorkspaceStats | null>(null);

	async function loadStats() {
		try {
			stats = await fetchWorkspaceStats(1);
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
		}
	}

	$effect(() => {
		if (!live.ready) return;
		loadStats();
		const timer = setInterval(loadStats, STATS_EVERY_MS);
		return () => clearInterval(timer);
	});

	const inStep = $derived(
		view.units.filter((u) => {
			const states = Object.values(u.hosts);
			return states.length > 0 && states.every((s) => s.state === 'in_step');
		}).length
	);
	const waiting = $derived(view.new_dirs.filter((d) => d.state === 'waiting'));
	const needs = $derived(view.conflicts.length + view.held.length + waiting.length);
	const today = $derived(view.growth.at(-1)?.[1] ?? 0);
	const hosts = $derived(view.hosts.filter((h) => h.status).map((h) => h.host));
	const traffic = $derived(stats ? trafficByHost(stats.traffic) : {});
	const dayBytes = $derived(
		Object.values(traffic).reduce((n, t) => n + t.bytes_in + t.bytes_out, 0)
	);
	const dayCpu = $derived(
		Object.values(stats ? costByHost(stats.cost) : {}).reduce(
			(n, c) => n + c.cpu_ms + c.git_cpu_ms,
			0
		)
	);
	const recent = $derived(
		[...view.units]
			.filter((u) => u.version > 0)
			.sort((a, b) => b.at - a.at)
			.slice(0, 8)
	);
	const worstRtt = $derived(Math.max(0, ...view.hosts.map((h) => h.rtt_ms ?? 0)));
	/** How long the day's cost rows span: a hub that started an hour ago has an hour of them. */
	const costSpan = $derived.by(() => {
		const first = Math.min(...(stats?.cost ?? []).map((c) => c.at));
		return Number.isFinite(first)
			? Math.min(DAY_MS, Math.max(3_600_000, live.now - first))
			: DAY_MS;
	});
</script>

<div class="flex flex-col gap-5">
	<Tiles>
		<StatTile name="Units" value={view.units.length} sub="{inStep} in step on every machine" />
		<StatTile
			name="Machines"
			value="{view.hosts.filter((h) => h.connected && h.status).length} of {view.hosts.filter(
				(h) => h.status
			).length}"
			sub={view.hosts.some((h) => h.rtt_ms !== null)
				? `connected · link up to ${fmtRtt(worstRtt)}`
				: 'connected'}
		/>
		<StatTile
			name="Needs you"
			value={needs}
			sub="{view.conflicts.length} conflicts, {view.held.length} held, {waiting.length} new"
		/>
		<StatTile
			name="On the hub"
			value={fmtBytes(view.bytes)}
			sub="+{fmtBytes(today)} pushed today"
		/>
		<StatTile
			name="Last 24 hours"
			value={stats ? fmtBytes(dayBytes) : '…'}
			sub={stats
				? `moved · ${(dayCpu / 1000).toFixed(0)} s CPU (${fmtPct(corePct(dayCpu, costSpan * Math.max(1, hosts.length)))} of a core per machine)`
				: 'traffic and cost'}
		/>
	</Tiles>

	<section class="flex flex-col gap-2">
		<h2 class="text-base font-semibold">Needs you</h2>
		{#if !needs}
			<p class="text-sm text-muted-foreground">
				Nothing: no conflict, no held snapshot, no new directory waiting.
			</p>
		{:else}
			<div class="flex flex-col gap-4">
				{#if view.held.length}<HeldList held={view.held} {onchange} />{/if}
				{#if view.conflicts.length}<WsConflicts conflicts={view.conflicts} {onchange} />{/if}
				{#if waiting.length}<NewDirs dirs={waiting} {onchange} />{/if}
			</div>
		{/if}
	</section>

	<HostsStrip hosts={view.hosts} {onchange} />

	<div class="grid gap-3 lg:grid-cols-[3fr_2fr]">
		<Figure
			title="Traffic, last 24 hours"
			sub="Bytes between each machine and the hub, per hour"
			legend={hosts.map((h) => ({ label: h, color: hostColor(hosts, h) }))}
			empty={stats && !stats.traffic.length && 'No transfer recorded yet.'}
		>
			{#if stats}
				<TrafficChart {stats} {hosts} height={170} />
			{:else}
				<p class="text-sm text-muted-foreground">Loading…</p>
			{/if}
		</Figure>
		<section class="flex flex-col gap-2 rounded-lg border bg-card p-3">
			<div class="text-sm font-medium">Changed last</div>
			{#if !recent.length}
				<p class="text-xs text-muted-foreground">No push yet.</p>
			{:else}
				<ul class="flex flex-col divide-y text-sm">
					{#each recent as u (u.unit)}
						<li class="flex items-baseline gap-2 py-1">
							<a
								href={unitHref(u.unit)}
								class="min-w-0 flex-1 truncate font-mono text-xs hover:underline">{u.unit}</a
							>
							<span class="text-xs text-muted-foreground tabular-nums"
								>{plural(u.version, 'push', 'pushes')} · {fmtStamp(u.at)}</span
							>
						</li>
					{/each}
				</ul>
			{/if}
		</section>
	</div>
</div>
