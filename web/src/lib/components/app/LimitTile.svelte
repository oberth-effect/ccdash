<script lang="ts">
	// A limit: how much is used, when it resets and where the pace leads,
	// over its window so far (`LimitSpark`). The text keeps to the top left,
	// where the rising line is not. A window that reset with no reading since
	// says so rather than showing the old number. The whole tile goes to
	// Analytics › Limits, which has the numbers as tables.
	import type { Limit, LimitTrack, ScopedLimit } from '#lib/api/types.js';
	import { fmtDuration, meterLevel } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import LimitSpark from './LimitSpark.svelte';
	import Meter from './Meter.svelte';

	let {
		name,
		limit,
		track
	}: {
		name: string;
		limit: Limit | ScopedLimit | null | undefined;
		track?: LimitTrack;
	} = $props();

	const COLOR = {
		normal: 'var(--working)',
		warning: 'var(--warning)',
		critical: 'var(--critical)'
	};

	const pct = $derived(limit ? Math.round(limit.used_pct) : 0);
	const level = $derived(meterLevel(pct));
	const resetsIn = $derived(limit ? limit.resets_at * 1000 - live.now : 0);
	const stale = $derived(Boolean(limit) && resetsIn <= 0);
	const forecast = $derived(track?.forecast ?? null);
	const atReset = $derived(forecast ? (forecast.at_reset_recent ?? forecast.at_reset_avg) : null);
	const outlook = $derived.by(() => {
		if (forecast?.full_at) return `full in ${fmtDuration(forecast.full_at - live.now)}`;
		return atReset !== null ? `≈ ${Math.round(atReset)}% at reset` : '';
	});
</script>

<a
	href="/analytics/limits"
	class="relative isolate flex min-h-24 flex-col gap-0.5 overflow-hidden rounded-lg border bg-card p-3 transition-colors hover:border-primary"
>
	{#if limit && track && !stale}
		<LimitSpark {track} used={limit.used_pct} now={live.now} color={COLOR[level]} />
	{/if}
	<span class="limit-text text-xs text-muted-foreground">{name}</span>
	{#if !limit}
		<span class="relative text-lg text-muted-foreground">–</span>
		<span class="relative text-xs text-muted-foreground">no statusline data yet</span>
	{:else if stale}
		<span class="relative text-lg text-muted-foreground">–</span>
		<span class="relative text-xs text-muted-foreground">reset, no reading since</span>
	{:else}
		<span class="limit-text text-lg font-semibold tabular-nums"
			>{pct}%{level === 'normal' ? '' : ' ⚠︎'}</span
		>
		{#if !track}<Meter {pct} label={name} class="relative w-full" />{/if}
		<span class="limit-text text-xs text-muted-foreground">resets in {fmtDuration(resetsIn)}</span>
		{#if outlook}
			<span class="limit-text text-xs text-muted-foreground">{outlook}</span>
		{/if}
	{/if}
</a>
