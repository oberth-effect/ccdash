<script lang="ts">
	// Workspace › Statistics: over a day, a week, a month or a quarter —
	// what moved, what it cost in CPU and memory per machine, the hub's
	// repositories, conflicts.
	import { Unauthorized } from '#lib/api/client.js';
	import {
		fetchWorkspaceStats,
		type CostRow,
		type WorkspaceStats,
		type WorkspaceView
	} from '#lib/api/index.js';
	import BarRows from '#lib/components/charts/BarRows.svelte';
	import Columns from '#lib/components/charts/Columns.svelte';
	import Figure from '#lib/components/charts/Figure.svelte';
	import SortableTable from '#lib/components/charts/SortableTable.svelte';
	import StatTile from '#lib/components/charts/StatTile.svelte';
	import Tiles from '#lib/components/charts/Tiles.svelte';
	import TimeLines from '#lib/components/charts/TimeLines.svelte';
	import { COLOR, dayFmt, fmtCount, type SortColumn } from '#lib/charts.js';
	import { fmtBytes, fmtDuration, fmtStamp, fmtTime, plural } from '#lib/format.js';
	import { untrack } from 'svelte';
	import { live } from '#lib/live/store.svelte.js';
	import { rangeLabel, reachesNow } from '#lib/range.js';
	import { rangeState } from '#lib/range.svelte.js';
	import {
		bucketStarts,
		corePct,
		costByHost,
		fmtPct,
		hostColor,
		trafficByHost,
		unitHref
	} from '#lib/workspace.js';
	import TrafficChart from './TrafficChart.svelte';

	let { view }: { view: WorkspaceView } = $props();

	let stats = $state<WorkspaceStats | null>(null);
	let error = $state('');
	let loading = $state(false);

	async function load(params: { since: number; until?: number }) {
		loading = true;
		try {
			const next = await fetchWorkspaceStats(params);
			// NOTE: a reply for a range the reader has left is dropped
			if (params.since === rangeState.params().since) stats = next;
			error = '';
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		if (!live.ready) return;
		const range = rangeState.current;
		const params = rangeState.params();
		untrack(() => load(params));
		if (!reachesNow(range, rangeState.now())) return;
		const timer = setInterval(() => load(rangeState.params()), 120_000);
		return () => clearInterval(timer);
	});

	const hosts = $derived(
		[
			...new Set([
				...view.hosts.filter((h) => h.status).map((h) => h.host),
				...(stats?.cost ?? []).map((r) => r.host),
				...(stats?.traffic ?? []).map((r) => r.host)
			])
		].sort()
	);
	const daily = $derived((stats?.bucket_ms ?? 0) >= 86_400_000);
	const label = (at: number) => (daily ? dayFmt.format(at) : fmtTime(at));
	const when = (at: number) => (daily ? dayFmt.format(at) : `${dayFmt.format(at)} ${fmtTime(at)}`);

	type Col = { key: string; label: string; at: number; by: Record<string, number> };
	/** Buckets of the span filled from `rows`, summing `pick` by host. */
	function buckets<T extends { host: string; at: number }>(
		rows: T[],
		pick: (r: T) => number
	): Col[] {
		if (!stats) return [];
		const out: Col[] = bucketStarts(stats, Date.now()).map((at) => ({
			key: String(at),
			label: label(at),
			at,
			by: {}
		}));
		const byKey = Object.fromEntries(out.map((c) => [c.key, c]));
		for (const r of rows) {
			const c = byKey[String(r.at)];
			if (c) c.by[r.host] = (c.by[r.host] ?? 0) + pick(r);
		}
		return out;
	}
	const hostSeries = (value: (h: string) => (d: Col) => number) =>
		hosts.map((h) => ({ key: h, label: h, color: hostColor(hosts, h), value: value(h) }));
	const total = (d: Col) => Object.values(d.by).reduce((n, v) => n + v, 0);

	const pushCols = $derived(buckets(stats?.pushes ?? [], (r) => r.pushes));
	const cpuCols = $derived(buckets(stats?.cost ?? [], (r) => (r.cpu_ms + r.git_cpu_ms) / 1000));
	const conflictCols = $derived.by(() => {
		if (!stats) return [];
		const rows = stats.conflicts.map(([at, n]) => ({ host: 'all', at, n }));
		return buckets(rows, (r) => r.n);
	});
	const memSeries = $derived(
		hosts.map((h) => ({
			key: h,
			label: h,
			color: hostColor(hosts, h),
			points: (stats?.cost ?? [])
				.filter((r) => r.host === h && r.rss_kb > 0)
				.map((r) => [r.at, r.rss_kb / 1024] as [number, number])
		}))
	);
	const memByTs = $derived.by(() => {
		const out: Record<number, CostRow[]> = {};
		for (const r of stats?.cost ?? []) (out[r.at] ??= []).push(r);
		return out;
	});

	const traffic = $derived(stats ? trafficByHost(stats.traffic) : {});
	const cost = $derived(stats ? costByHost(stats.cost) : {});
	const spanMs = $derived(stats ? Math.max(1, stats.until - stats.since) : 1);
	const totalBytes = $derived(
		Object.values(traffic).reduce((n, t) => n + t.bytes_in + t.bytes_out, 0)
	);
	const totalCpuMs = $derived(Object.values(cost).reduce((n, c) => n + c.cpu_ms + c.git_cpu_ms, 0));
	const totalPushes = $derived((stats?.pushes ?? []).reduce((n, p) => n + p.pushes, 0));

	type HostRow = Record<string, unknown> & {
		host: string;
		pushes: number;
		fetches: number;
		up: number;
		down: number;
		cpu_s: number;
		share: number;
		io: number;
		rss: number;
		failures: number;
	};
	const hostRows = $derived<HostRow[]>(
		hosts.map((h) => {
			const t = traffic[h];
			const c = cost[h];
			return {
				host: h,
				pushes: c?.pushes ?? t?.pushes ?? 0,
				fetches: c?.fetches ?? 0,
				up: t?.bytes_in ?? 0,
				down: t?.bytes_out ?? 0,
				cpu_s: ((c?.cpu_ms ?? 0) + (c?.git_cpu_ms ?? 0)) / 1000,
				share: corePct((c?.cpu_ms ?? 0) + (c?.git_cpu_ms ?? 0), spanMs),
				io: (c?.io_read ?? 0) + (c?.io_write ?? 0),
				rss: (c?.rss_kb ?? 0) * 1024,
				failures: (c?.failures ?? 0) + (t?.failures ?? 0)
			};
		})
	);
	const hostColumns: SortColumn<HostRow>[] = [
		{ key: 'host', label: 'Machine', text: true },
		{ key: 'pushes', label: 'Pushes' },
		{ key: 'fetches', label: 'Fetches' },
		{ key: 'up', label: 'Sent', fmt: (v) => fmtBytes(Number(v)) },
		{ key: 'down', label: 'Received', fmt: (v) => fmtBytes(Number(v)) },
		{ key: 'cpu_s', label: 'CPU', fmt: (v) => `${Number(v).toFixed(1)} s` },
		{ key: 'share', label: 'Of one core', fmt: (v) => fmtPct(Number(v)) },
		{ key: 'io', label: 'Disk I/O', fmt: (v) => fmtBytes(Number(v)) },
		{ key: 'rss', label: 'Peak resident', fmt: (v) => fmtBytes(Number(v)) },
		{ key: 'failures', label: 'Failures' }
	];

	const repoRows = $derived(
		(stats?.repos ?? []).slice(0, 15).map((r) => ({
			label: r.unit,
			total: r.bytes,
			parts: [[r.bytes, COLOR.neutral]] as [number, string][],
			unit: r.unit
		}))
	);
	const repoTable = $derived(
		(stats?.repos ?? []).map((r) => ({ unit: r.unit, bytes: r.bytes }) as Record<string, unknown>)
	);
</script>

<div class="flex flex-col gap-5">
	<p class="text-sm text-muted-foreground">
		What the sync moved and cost, per machine, and what the hub holds, over the range in the bar ({rangeLabel(
			rangeState.current
		)}). Hourly buckets up to three days, daily beyond; the cost is the collectors' sync threads
		plus the git processes they ran.
	</p>
	{#if error}<p class="text-sm text-critical" role="alert">{error}</p>{/if}
	{#if !stats}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{:else}
		<div class="flex flex-col gap-3 transition-opacity" class:opacity-60={loading}>
			<Tiles>
				<StatTile
					name="Moved"
					value={fmtBytes(totalBytes)}
					sub="between the machines and the hub"
				/>
				<StatTile
					name="CPU"
					value="{(totalCpuMs / 1000).toFixed(0)} s"
					sub="{fmtPct(
						corePct(totalCpuMs, spanMs * Math.max(1, hosts.length))
					)} of one core per machine"
				/>
				<StatTile name="Pushes" value={totalPushes} sub="snapshots that reached the hub" />
				<StatTile
					name="On the hub"
					value={fmtBytes(stats.hub_bytes)}
					sub={plural(stats.repos.length, 'repository', 'repositories')}
				/>
			</Tiles>

			<Figure
				title="Per machine"
				sub="Over the range"
				empty={!hostRows.length && 'No machine reported yet.'}
			>
				<SortableTable columns={hostColumns} rows={hostRows} sortKey="cpu_s" />
			</Figure>

			<div class="grid gap-3 lg:grid-cols-2">
				<Figure
					title="Traffic"
					sub="Bytes per {daily ? 'day' : 'hour'}, stacked by machine"
					legend={hosts.map((h) => ({ label: h, color: hostColor(hosts, h) }))}
					empty={!stats.traffic.length && 'No transfer recorded in the range.'}
				>
					<TrafficChart {stats} {hosts} />
				</Figure>
				<Figure
					title="Pushes"
					sub="Snapshots that reached the hub per {daily ? 'day' : 'hour'}"
					legend={hosts.map((h) => ({ label: h, color: hostColor(hosts, h) }))}
					empty={!stats.pushes.length && 'No push in the range.'}
				>
					<Columns
						data={pushCols}
						series={hostSeries((h) => (d) => d.by[h] ?? 0)}
						fmtY={fmtCount}
						tip={(d) => [
							plural(total(d), 'push', 'pushes'),
							when(d.at),
							...hosts.filter((h) => d.by[h]).map((h) => `${h} ${d.by[h]}`)
						]}
					/>
				</Figure>
				<Figure
					title="CPU"
					sub="Seconds per {daily ? 'day' : 'hour'}, the sync's threads and its git processes"
					legend={hosts.map((h) => ({ label: h, color: hostColor(hosts, h) }))}
					empty={!stats.cost.length && 'No cost recorded in the range.'}
				>
					<Columns
						data={cpuCols}
						series={hostSeries((h) => (d) => d.by[h] ?? 0)}
						fmtY={(v) => `${Number(v.toFixed(1))} s`}
						tip={(d) => [
							`${total(d).toFixed(1)} s CPU`,
							when(d.at),
							`${fmtPct(corePct(total(d) * 1000, stats?.bucket_ms ?? 1))} of one core, all machines`,
							...hosts
								.filter((h) => d.by[h])
								.map(
									(h) =>
										`${h} ${d.by[h].toFixed(1)} s (${fmtPct(corePct(d.by[h] * 1000, stats?.bucket_ms ?? 1))})`
								)
						]}
					/>
				</Figure>
				<Figure
					title="Memory"
					sub="The collector's peak resident set per {daily ? 'day' : 'hour'} (the whole process)"
					legend={hosts.map((h) => ({ label: h, color: hostColor(hosts, h), line: true }))}
					empty={!memSeries.some((s) => s.points.length) && 'No reading in the range.'}
				>
					<TimeLines
						series={memSeries}
						since={stats.since}
						until={live.now}
						fmt={(v) => `${Math.round(v)} MB`}
						height={160}
						tip={(ts) => [
							when(ts),
							...(memByTs[ts] ?? []).map(
								(r) => `${r.host} ${fmtBytes(r.rss_kb * 1024)} resident, ${r.watches} watches`
							)
						]}
					/>
				</Figure>
				<Figure
					title="The hub's repositories"
					sub="The largest units; every unit in the table"
					empty={!stats.repos.length && 'No repository yet.'}
					table={{
						headers: ['Unit', 'Size'],
						rows: repoTable.map((r) => [String(r.unit), fmtBytes(Number(r.bytes))])
					}}
				>
					<BarRows
						rows={repoRows}
						valueText={(row) => fmtBytes(row.total)}
						tip={(row) => [fmtBytes(row.total), row.unit]}
					/>
					<p class="text-xs text-muted-foreground">
						A unit's page shows its snapshots: <a
							href={unitHref(repoRows[0]?.unit ?? '')}
							class="hover:underline">the largest</a
						>
						is {repoRows[0]?.unit ?? '—'}.
					</p>
				</Figure>
				<Figure
					title="Conflicts"
					sub="Recorded per {daily ? 'day' : 'hour'}"
					empty={!stats.conflicts.length && 'No conflict in the range.'}
				>
					<Columns
						data={conflictCols}
						series={[
							{
								key: 'all',
								label: 'conflicts',
								color: COLOR.permission,
								value: (d) => d.by.all ?? 0
							}
						]}
						fmtY={fmtCount}
						tip={(d) => [plural(d.by.all ?? 0, 'conflict'), when(d.at)]}
					/>
				</Figure>
			</div>
			<p class="text-xs text-muted-foreground">
				{fmtDuration(stats.until - stats.since)} ending {fmtStamp(stats.until)}; rows are kept for a
				year.
			</p>
		</div>
	{/if}
</div>
