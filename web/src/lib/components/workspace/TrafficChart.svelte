<script lang="ts">
	// Bytes per bucket, stacked by machine, from the stats' traffic rows.
	import type { WorkspaceStats } from '#lib/api/types.js';
	import Columns from '#lib/components/charts/Columns.svelte';
	import { dayFmt } from '#lib/charts.js';
	import { fmtBytes, fmtTime } from '#lib/format.js';
	import { bucketStarts, hostColor } from '#lib/workspace.js';

	let {
		stats,
		hosts,
		height = 190
	}: { stats: WorkspaceStats; hosts: string[]; height?: number } = $props();

	type Col = { key: string; label: string; at: number; by: Record<string, number> };

	const daily = $derived(stats.bucket_ms >= 86_400_000);
	const label = (at: number) => (daily ? dayFmt.format(at) : fmtTime(at));
	/** Every bucket of the span, so quiet hours show as gaps, not as missing columns. */
	const cols = $derived.by(() => {
		const out: Col[] = bucketStarts(stats, Date.now()).map((at) => ({
			key: String(at),
			label: label(at),
			at,
			by: {}
		}));
		const byKey = Object.fromEntries(out.map((c) => [c.key, c]));
		for (const r of stats.traffic) {
			const c = byKey[String(r.at)];
			if (c) c.by[r.host] = (c.by[r.host] ?? 0) + r.bytes_in + r.bytes_out;
		}
		return out;
	});
	const series = $derived(
		hosts.map((h) => ({
			key: h,
			label: h,
			color: hostColor(hosts, h),
			value: (d: Col) => d.by[h] ?? 0
		}))
	);
</script>

<Columns
	data={cols}
	{series}
	{height}
	fmtY={(v) => fmtBytes(v)}
	tip={(d) => [
		fmtBytes(Object.values(d.by).reduce((n, v) => n + v, 0)),
		`${daily ? dayFmt.format(d.at) : `${dayFmt.format(d.at)} ${fmtTime(d.at)}`}`,
		...hosts.filter((h) => d.by[h]).map((h) => `${h} ${fmtBytes(d.by[h])}`)
	]}
/>
