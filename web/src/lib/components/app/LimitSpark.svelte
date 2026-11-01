<script lang="ts">
	// The window of a limit behind its tile: x is the window, y 0–100 %. The
	// readings so far as a step line (a value holds until the next reading),
	// where the recent pace leads, and the even pace as the faint diagonal:
	// above it the limit is going faster than the window. Hovering reads the
	// value off the line.
	import type { LimitTrack } from '#lib/api/types.js';
	import { fmtStamp, fmtTime } from '#lib/format.js';

	const HOUR_MS = 3_600_000;

	let { track, used, now, color }: { track: LimitTrack; used: number; now: number; color: string } =
		$props();

	let hover = $state<{ x: number; text: string } | null>(null);

	const forecast = $derived(track.forecast);
	const start = $derived(forecast ? forecast.resets_at - forecast.window_ms : 0);
	const span = $derived(forecast ? forecast.window_ms : 1);
	const xOf = (ts: number) => Math.min(100, Math.max(0, ((ts - start) / span) * 100));
	const yOf = (pct: number) => 100 - Math.min(100, Math.max(0, pct));

	/** The readings with the live value at `now`. */
	const points = $derived<[number, number][]>([...track.points, [now, used]]);
	const line = $derived.by(() => {
		const [first, ...rest] = points;
		if (!first) return '';
		let path = `M${xOf(first[0]).toFixed(2)} ${yOf(first[1]).toFixed(2)}`;
		for (const [ts, pct] of rest) path += `H${xOf(ts).toFixed(2)}V${yOf(pct).toFixed(2)}`;
		return path;
	});
	const area = $derived(points.length ? `${line}V100H${xOf(points[0][0]).toFixed(2)}Z` : '');
	const atReset = $derived(forecast ? (forecast.at_reset_recent ?? forecast.at_reset_avg) : null);

	function read(event: PointerEvent) {
		const box = (event.currentTarget as SVGElement).getBoundingClientRect();
		const x = Math.min(1, Math.max(0, (event.clientX - box.left) / box.width));
		const ts = start + x * span;
		if (ts > now) return void (hover = null);
		const last = points.findLast((p) => p[0] <= ts);
		if (!last) return void (hover = null);
		const when = span > 24 * HOUR_MS ? fmtStamp(ts) : fmtTime(ts);
		hover = { x: x * 100, text: `${when} · ${Math.round(last[1])}%` };
	}
</script>

{#if forecast}
	<svg
		class="absolute inset-0 size-full"
		viewBox="0 0 100 100"
		preserveAspectRatio="none"
		role="img"
		aria-label="{Math.round(used)}% used over this window"
		onpointermove={read}
		onpointerleave={() => (hover = null)}
	>
		<line x1="0" y1="100" x2="100" y2="0" class="diagonal" />
		<path d={area} fill={color} opacity="0.16" />
		<path d={line} fill="none" stroke={color} class="trace" />
		{#if atReset !== null}
			<line
				x1={xOf(now)}
				y1={yOf(used)}
				x2="100"
				y2={yOf(atReset)}
				stroke={color}
				class="projection"
			/>
		{/if}
		<line x1={xOf(now)} y1="0" x2={xOf(now)} y2="100" class="now" />
		{#if hover}<line x1={hover.x} y1="0" x2={hover.x} y2="100" class="now" />{/if}
	</svg>
	{#if hover}
		<span
			class="pointer-events-none absolute top-1.5 right-2 rounded bg-popover px-1.5 py-0.5 text-xs tabular-nums shadow"
			>{hover.text}</span
		>
	{/if}
{/if}

<style>
	svg * {
		vector-effect: non-scaling-stroke;
	}
	.diagonal {
		stroke: var(--muted-foreground);
		stroke-width: 1px;
		stroke-dasharray: 2 4;
		opacity: 0.35;
	}
	.trace {
		stroke-width: 2px;
		stroke-linejoin: round;
	}
	.projection {
		stroke-width: 1.5px;
		stroke-dasharray: 4 3;
		opacity: 0.7;
	}
	.now {
		stroke: var(--muted-foreground);
		stroke-width: 1px;
		opacity: 0.45;
	}
</style>
