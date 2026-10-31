// The Workspace page's vocabulary: unit states with their colour and
// label, unit ids as links.

import type {
	CostRow,
	Decision,
	TrafficRow,
	UnitState,
	UnitStatus,
	UnitView,
	WorkspaceHost
} from './api/types';
import { COLOR } from './charts';
import { fmtDuration } from './format';

/** The area's sections, in tab order. */
export const SECTIONS = [
	['overview', 'Overview'],
	['units', 'Units'],
	['machines', 'Machines'],
	['stats', 'Statistics'],
	['config', 'Configuration']
] as const;
export type Section = (typeof SECTIONS)[number][0];

export const STATE: Record<UnitState, { label: string; dot: string; busy?: boolean }> = {
	in_step: { label: 'in step', dot: 'bg-your-turn' },
	ahead: { label: 'sending', dot: 'bg-working', busy: true },
	behind: { label: 'receiving', dot: 'bg-working', busy: true },
	merging: { label: 'merging', dot: 'bg-working', busy: true },
	conflict: { label: 'conflict', dot: 'bg-critical' },
	held: { label: 'held', dot: 'bg-permission' },
	paused: { label: 'paused', dot: 'bg-idle' },
	skipped: { label: 'waiting', dot: 'bg-parked' },
	waiting: { label: 'waiting', dot: 'bg-parked' },
	failed: { label: 'failing', dot: 'bg-critical' },
	seeding: { label: 'seeding', dot: 'bg-working', busy: true },
	unadopted: { label: 'to adopt', dot: 'bg-permission' },
	creating: { label: 'creating', dot: 'bg-working', busy: true },
	missing: { label: 'missing', dot: 'bg-idle' }
};

/** States that call for the owner. */
export const ATTENTION: UnitState[] = ['conflict', 'held', 'failed', 'unadopted'];

/** The unit page. */
export const unitHref = (unit: string) =>
	`/workspace/unit/${unit.split('/').map(encodeURIComponent).join('/')}`;

/** One colour per machine, stable across the charts of a page. */
const HOST_COLORS = [
	COLOR.working,
	COLOR.your_turn,
	COLOR.permission,
	COLOR.tools,
	COLOR.cache_write,
	COLOR.output
];
export const hostColor = (hosts: string[], host: string) =>
	HOST_COLORS[Math.max(0, hosts.indexOf(host)) % HOST_COLORS.length];

/** What the globs decided, in words. */
export function decisionText(d: Decision): string {
	switch (d.kind) {
		case 'selected':
			return d.rule ? `matched by ${d.rule}` : 'every directory travels';
		case 'forced_in':
			return 'included by you';
		case 'forced_out':
			return 'excluded by you';
		case 'excluded':
			return `excluded by ${d.rule}`;
		case 'unmatched':
			return 'matched by no include glob';
	}
}

/** Coverage statuses, in words. */
export const COVERAGE_STATUS: Record<string, string> = {
	travels: 'travels',
	waiting: 'over the threshold, waits for you',
	ignored: 'ignored by you',
	unsyncable: 'cannot travel',
	left_out: 'left out'
};

/** Totals of traffic rows (both directions) by host. */
export function trafficByHost(rows: TrafficRow[]) {
	const out: Record<
		string,
		{
			requests: number;
			bytes_in: number;
			bytes_out: number;
			ms: number;
			failures: number;
			pushes: number;
			push_ms: number;
		}
	> = {};
	for (const r of rows) {
		const t = (out[r.host] ??= {
			requests: 0,
			bytes_in: 0,
			bytes_out: 0,
			ms: 0,
			failures: 0,
			pushes: 0,
			push_ms: 0
		});
		t.requests += r.requests;
		t.bytes_in += r.bytes_in;
		t.bytes_out += r.bytes_out;
		t.ms += r.ms;
		t.failures += r.failures;
		if (r.kind === 'push') {
			t.pushes += r.requests;
			t.push_ms += r.ms;
		}
	}
	return out;
}

/** Totals of cost rows by host. */
export function costByHost(rows: CostRow[]) {
	const out: Record<string, CostRow> = {};
	for (const r of rows) {
		const c = (out[r.host] ??= { ...r, rss_kb: 0, watches: 0 });
		if (c !== r) {
			c.cpu_ms += r.cpu_ms;
			c.git_cpu_ms += r.git_cpu_ms;
			c.git_calls += r.git_calls;
			c.git_wall_ms += r.git_wall_ms;
			c.io_read += r.io_read;
			c.io_write += r.io_write;
			c.passes += r.passes;
			c.snapshots += r.snapshots;
			c.pushes += r.pushes;
			c.fetches += r.fetches;
			c.push_ms += r.push_ms;
			c.fetch_ms += r.fetch_ms;
			c.failures += r.failures;
		}
		c.rss_kb = Math.max(c.rss_kb, r.rss_kb);
		c.watches = Math.max(c.watches, r.watches);
	}
	return out;
}

/** CPU milliseconds over a span as a share of one core. */
export const corePct = (cpuMs: number, spanMs: number) => (spanMs > 0 ? (100 * cpuMs) / spanMs : 0);

export const fmtPct = (v: number) =>
	v >= 10 ? `${v.toFixed(0)} %` : v >= 1 ? `${v.toFixed(1)} %` : `${v.toFixed(2)} %`;

/** The bucket starts of a stats span: the hours as they are, local midnights
 * for daily buckets (what the hub's rows carry), from the first to now. */
export function bucketStarts(stats: { since: number; bucket_ms: number }, now: number): number[] {
	const out: number[] = [];
	if (stats.bucket_ms >= 86_400_000) {
		const d = new Date(stats.since);
		d.setHours(0, 0, 0, 0);
		for (; d.getTime() <= now; d.setDate(d.getDate() + 1)) out.push(d.getTime());
	} else {
		const first = stats.since - (stats.since % stats.bucket_ms);
		for (let at = first; at <= now; at += stats.bucket_ms) out.push(at);
	}
	return out;
}

/** A duration in ms, in ms while it is short. */
export const fmtMs = (ms: number) => (ms < 1000 ? `${Math.round(ms)} ms` : fmtDuration(ms));

/** A link's round trip; a LAN rounds to nothing. */
export const fmtRtt = (ms: number) => (ms < 1 ? '<1 ms' : `${ms} ms`);

/** Bytes per second, as the uplink question wants it. */
export const fmtRate = (bytes: number, ms: number) => {
	if (ms <= 0) return '—';
	const bps = (bytes * 1000) / ms;
	if (bps >= 1024 ** 2) return `${(bps / 1024 ** 2).toFixed(1)} MB/s`;
	if (bps >= 1024) return `${(bps / 1024).toFixed(0)} kB/s`;
	return `${bps.toFixed(0)} B/s`;
};

/** A one-line summary of a machine's collector. */
export function hostSummary(h: WorkspaceHost, now: number, staleMs = 600_000): string {
	const s = h.status;
	if (!s) return h.connected ? 'not syncing its workspace' : 'offline';
	const parts = [`${s.units} units`, `${s.in_step} in step`];
	if (s.pending) parts.push(`${s.pending} moving`);
	if (!s.watching) parts.push('no file watches, sweeping instead');
	if (s.last_pass && now - s.last_pass > staleMs)
		parts.push(`last pass ${Math.round((now - s.last_pass) / 60_000)} min ago`);
	if (!h.connected) parts.push('offline');
	return parts.join(' · ');
}

/** The first path component of a unit id. */
export const bundleOf = (unit: string) => unit.split('/')[0] ?? unit;

/** The unit's name within its bundle (the bundle root is `.`). */
export const unitName = (unit: string) => {
	const i = unit.indexOf('/');
	return i < 0 ? '.' : unit.slice(i + 1);
};

/** The worst state across a unit's hosts, for the row's chip and the filters. */
export function worst(u: UnitView): UnitState | null {
	const states = Object.values(u.hosts).map((s: UnitStatus) => s.state);
	if (!states.length) return null;
	for (const s of ATTENTION) if (states.includes(s)) return s;
	for (const s of ['seeding', 'creating', 'merging', 'ahead', 'behind'] as UnitState[])
		if (states.includes(s)) return s;
	for (const s of ['skipped', 'waiting', 'missing', 'paused'] as UnitState[])
		if (states.includes(s)) return s;
	return 'in_step';
}

/** A short commit id. */
export const short = (sha: string | null | undefined) => (sha ? sha.slice(0, 8) : '—');
