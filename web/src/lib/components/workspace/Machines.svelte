<script lang="ts">
	// Workspace › Machines: one card per collector (the link, the
	// collector, today's traffic and cost, the throughput of its large
	// transfers, Resync), the last day's traffic per machine and the recent
	// large transfers.
	import { Unauthorized } from '#lib/api/client.js';
	import {
		fetchWorkspaceStats,
		workspaceResync,
		type Transfer,
		type WorkspaceHost,
		type WorkspaceStats,
		type WorkspaceView
	} from '#lib/api/index.js';
	import Meter from '#lib/components/app/Meter.svelte';
	import Figure from '#lib/components/charts/Figure.svelte';
	import SortableTable from '#lib/components/charts/SortableTable.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import type { SortColumn } from '#lib/charts.js';
	import { fmtBytes, fmtDuration, fmtStamp, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import {
		corePct,
		costByHost,
		fmtMs,
		fmtPct,
		fmtRate,
		fmtRtt,
		hostColor,
		hostSummary,
		trafficByHost
	} from '#lib/workspace.js';
	import TrafficChart from './TrafficChart.svelte';

	let { view, onchange }: { view: WorkspaceView; onchange: () => void } = $props();

	const STATS_EVERY_MS = 60_000;
	const DAY_MS = 86_400_000;

	let day = $state<WorkspaceStats | null>(null);
	let week = $state<WorkspaceStats | null>(null);
	let busy = $state(false);
	let error = $state('');

	async function loadStats() {
		try {
			[day, week] = await Promise.all([fetchWorkspaceStats(1), fetchWorkspaceStats(7)]);
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		}
	}

	$effect(() => {
		if (!live.ready) return;
		loadStats();
		const timer = setInterval(loadStats, STATS_EVERY_MS);
		return () => clearInterval(timer);
	});

	async function resync(host: string) {
		busy = true;
		error = '';
		try {
			await workspaceResync(host);
			onchange();
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}

	const hosts = $derived(view.hosts.filter((h) => h.status).map((h) => h.host));
	const dayTraffic = $derived(day ? trafficByHost(day.traffic) : {});
	const weekTraffic = $derived(week ? trafficByHost(week.traffic) : {});
	const dayCost = $derived(day ? costByHost(day.cost) : {});
	const since = (h: WorkspaceHost) => live.hosts.find((x) => x.name === h.host)?.since ?? null;

	/** "connected for 6 min · 2 ms round trip", or since when it is offline. */
	function linkLine(h: WorkspaceHost): string {
		const at = since(h);
		if (!h.connected) return at ? `offline since ${fmtStamp(at)}` : 'offline';
		const parts = [at ? `connected for ${fmtDuration(live.now - at)}` : 'connected'];
		if (h.rtt_ms !== null) parts.push(`${fmtRtt(h.rtt_ms)} round trip`);
		return parts.join(' · ');
	}

	/** Median rate of a host's large transfers in the week, and how many. */
	function throughput(host: string): { n: number; rate: string; peak: string } {
		const mine = (week?.transfers ?? []).filter((t) => t.host === host && t.ms > 0);
		if (!mine.length) return { n: 0, rate: '—', peak: '—' };
		const rates = mine.map((t) => (t.bytes * 1000) / t.ms).sort((a, b) => a - b);
		const median = rates[rates.length >> 1];
		return {
			n: mine.length,
			rate: fmtRate(median, 1000),
			peak: fmtRate(rates[rates.length - 1], 1000)
		};
	}

	type Row = Record<string, unknown> & Transfer & { rate: number };
	const transferRows = $derived<Row[]>(
		(week?.transfers ?? []).map((t) => ({ ...t, rate: t.ms > 0 ? (t.bytes * 1000) / t.ms : 0 }))
	);
	const transferColumns: SortColumn<Row>[] = [
		{ key: 'at', label: 'When', fmt: (v) => fmtStamp(Number(v)) },
		{ key: 'host', label: 'Machine', text: true },
		{ key: 'unit', label: 'Unit', text: true },
		{
			key: 'kind',
			label: 'Direction',
			text: true,
			fmt: (v) => (v === 'push' ? 'pushed' : 'fetched')
		},
		{ key: 'bytes', label: 'Size', fmt: (v) => fmtBytes(Number(v)) },
		{ key: 'ms', label: 'Took', fmt: (v) => fmtMs(Number(v)) },
		{ key: 'rate', label: 'Rate', fmt: (v) => fmtRate(Number(v), 1000) }
	];
</script>

<div class="flex flex-col gap-5">
	<p class="text-sm text-muted-foreground">
		Each machine's collector, its link to the hub and what the sync moves and costs there. The rate
		of the large transfers tells whether an uplink limits a machine; the cost figures are the sync's
		own threads plus the git processes it ran.
	</p>
	{#if error}<p class="text-sm text-critical" role="alert">{error}</p>{/if}
	<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
		{#each view.hosts as h (h.host)}
			{@const t = dayTraffic[h.host]}
			{@const w = weekTraffic[h.host]}
			{@const c = dayCost[h.host]}
			{@const cost = h.status?.cost ?? null}
			{@const tp = throughput(h.host)}
			<section class="flex flex-col gap-2 rounded-lg border bg-card p-3 text-sm">
				<div class="flex items-center gap-2">
					<i
						class="inline-block size-2.5 shrink-0 rounded-full"
						style:background={h.connected ? hostColor(hosts, h.host) : 'var(--critical)'}
						aria-hidden="true"
					></i>
					<span class="font-semibold">{h.host}</span>
					<span class="text-xs text-muted-foreground">{linkLine(h)}</span>
					<Button
						variant="outline"
						size="xs"
						class="ml-auto"
						disabled={busy || !h.connected || !h.status}
						onclick={() => resync(h.host)}>Resync</Button
					>
				</div>
				{#if h.status}
					<div class="truncate font-mono text-xs text-muted-foreground">{h.status.root}</div>
					<div class="text-xs text-muted-foreground">{hostSummary(h, live.now)}</div>
					{#if h.status.progress}
						{@const [done, total] = h.status.progress}
						<div class="flex items-center gap-2 text-xs text-muted-foreground">
							<Meter pct={total ? (done / total) * 100 : 0} level="neutral" class="w-28" />
							{done} of {total} units seeded or created
						</div>
					{/if}
					{#if h.status.error}<div class="text-xs text-critical">{h.status.error}</div>{/if}
				{:else}
					<div class="text-xs text-muted-foreground">
						{h.connected ? 'not syncing its workspace' : 'offline'}
					</div>
				{/if}
				<dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs">
					<dt class="text-muted-foreground">Today</dt>
					<dd class="tabular-nums">
						{#if t}
							↑ {fmtBytes(t.bytes_in)} · ↓ {fmtBytes(t.bytes_out)} · {plural(
								t.requests,
								'request'
							)}{#if t.pushes}, a push took {fmtMs(t.push_ms / t.pushes)} on average{/if}{#if t.failures}
								· <span class="text-critical">{plural(t.failures, 'failure')}</span>{/if}
						{:else if day}
							nothing moved
						{:else}
							…
						{/if}
					</dd>
					<dt class="text-muted-foreground">7 days</dt>
					<dd class="tabular-nums">
						{#if w}
							↑ {fmtBytes(w.bytes_in)} · ↓ {fmtBytes(w.bytes_out)} · {plural(w.requests, 'request')}
						{:else if week}
							nothing moved
						{:else}
							…
						{/if}
					</dd>
					<dt class="text-muted-foreground">Large transfers</dt>
					<dd class="tabular-nums">
						{#if tp.n}
							{tp.n} in 7 days · median {tp.rate} · best {tp.peak}
						{:else}
							none over 64 kB yet
						{/if}
					</dd>
					<dt class="text-muted-foreground">Cost today</dt>
					<dd class="tabular-nums">
						{#if c}
							{((c.cpu_ms + c.git_cpu_ms) / 1000).toFixed(1)} s CPU ({fmtPct(
								corePct(
									c.cpu_ms + c.git_cpu_ms,
									Math.min(DAY_MS, live.now - (day?.since ?? live.now)) || DAY_MS
								)
							)} of one core) · {plural(c.passes, 'pass', 'passes')}, {plural(
								c.git_calls,
								'git call'
							)} · read {fmtBytes(c.io_read)}, wrote {fmtBytes(c.io_write)}
						{:else if day}
							nothing recorded yet
						{:else}
							…
						{/if}
					</dd>
					{#if cost}
						<dt class="text-muted-foreground">Now</dt>
						<dd class="tabular-nums">
							{fmtBytes(cost.rss_kb * 1024)} resident · {plural(cost.watches, 'watch', 'watches')} · since
							start {((cost.cpu_ms + cost.git_cpu_ms) / 1000).toFixed(0)} s CPU, {plural(
								cost.pushes,
								'push',
								'pushes'
							)}, {plural(cost.fetches, 'fetch', 'fetches')}{#if cost.failures}, {plural(
									cost.failures,
									'failed pass',
									'failed passes'
								)}{/if}
						</dd>
					{/if}
				</dl>
			</section>
		{/each}
	</div>

	<Figure
		title="Traffic per machine, last 24 hours"
		sub="Bytes between each machine and the hub, per hour"
		legend={hosts.map((h) => ({ label: h, color: hostColor(hosts, h) }))}
		empty={day && !day.traffic.length && 'No transfer recorded yet.'}
	>
		{#if day}
			<TrafficChart stats={day} {hosts} />
		{:else}
			<p class="text-sm text-muted-foreground">Loading…</p>
		{/if}
	</Figure>

	<Figure
		title="Large transfers, last 7 days"
		sub="Every transfer over 64 kB with its rate; a low rate on one machine only points at its uplink"
		empty={week && !transferRows.length && 'No transfer over 64 kB yet.'}
	>
		{#if week}
			<SortableTable columns={transferColumns} rows={transferRows} sortKey="at" />
		{:else}
			<p class="text-sm text-muted-foreground">Loading…</p>
		{/if}
	</Figure>
</div>
