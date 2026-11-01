<script lang="ts">
	// A session as the Now page shows it: what was asked, what Claude last
	// said, the tail of its steps with the running one under them, tasks,
	// subagents, files and the turn's numbers, so a look replaces opening it.
	// `children` is what a session waiting on you can be answered with.
	import OctagonXIcon from '@lucide/svelte/icons/octagon-x';
	import type { Snippet } from 'svelte';
	import type { Lane, Session } from '#lib/api/types.js';
	import SubagentList from '#lib/components/session/SubagentList.svelte';
	import {
		fmtBytes,
		fmtDuration,
		fmtUsd,
		known,
		plain,
		plural,
		toolLabel,
		toolTitle
	} from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { children as agentsOf, label, quietFor, shown } from '#lib/session.js';
	import { base, family, ICONS, tellStep } from '#lib/steps.js';
	import Clock from './Clock.svelte';
	import Meter from './Meter.svelte';
	import SessionActions from './SessionActions.svelte';
	import StateBadge from './StateBadge.svelte';
	import Strip from './Strip.svelte';
	import Where from './Where.svelte';

	const STEPS = 4;
	const FILES = 3;

	let {
		session: s,
		lane,
		children
	}: { session: Session; lane?: Lane; children?: Snippet } = $props();

	const edge: Record<string, string> = {
		permission: 'border-l-permission',
		your_turn: 'border-l-your-turn',
		working: 'border-l-working'
	};

	const working = $derived(s.activity === 'working');
	const quiet = $derived(quietFor(s, live.now));
	const asked = $derived(plain(s.prompt?.text));
	const said = $derived(plain(working ? s.message?.text : s.reply?.text));
	const steps = $derived((s.turn?.steps ?? []).slice(-STEPS));
	const Running = $derived(s.tool ? ICONS[family(s.tool.name)] : null);
	const tasksDone = $derived(s.tasks.filter((t) => t.status === 'completed').length);
	const activeTask = $derived(s.tasks.find((t) => t.status === 'in_progress'));
	const agents = $derived(agentsOf(live.sessions.values(), s).length);
	const files = $derived(s.turn?.files ?? []);
	const turnMs = $derived.by(() => {
		const t = s.turn;
		return t ? (t.ended_at ?? live.now) - t.started_at : null;
	});
	const facts = $derived(
		[
			known(turnMs) ? `turn ${fmtDuration(turnMs)}` : null,
			s.turn?.tools ? plural(s.turn.tools, 'tool') : null,
			s.turn?.failures ? `${s.turn.failures} failed` : null,
			agents ? plural(agents, 'background agent') : null,
			s.lines_added || s.lines_removed ? `+${s.lines_added ?? 0} −${s.lines_removed ?? 0}` : null,
			s.cost_usd ? fmtUsd(s.cost_usd) : null,
			s.resources ? `${s.resources.cpu_pct}% CPU · ${fmtBytes(s.resources.mem_bytes)}` : null
		].filter(Boolean)
	);
</script>

<article
	class="flex min-w-0 flex-col gap-2 rounded-lg border border-l-2 bg-card p-3 text-sm {edge[
		shown(s)
	] ?? ''}"
	data-activity={shown(s)}
>
	<header class="flex min-w-0 items-center gap-2">
		<StateBadge activity={shown(s)} />
		<a href="/sessions/{s.id}" class="min-w-0 flex-1 truncate font-medium hover:underline"
			>{label(s)}</a
		>
		<Clock since={s.activity_since} class="shrink-0 text-xs" />
		{#if quiet !== null}
			<span class="shrink-0 text-xs font-medium text-warning" title="No hook for this long"
				>quiet {fmtDuration(quiet).split(' ')[0]}</span
			>
		{/if}
		{#if working}<SessionActions session={s} />{/if}
	</header>
	<Where session={s} />

	{#if !s.hooked}
		<p class="text-xs text-muted-foreground">
			Started before ccdash was installed, so only busy and idle are known.
		</p>
	{:else}
		{#if asked}
			<p class="line-clamp-2 text-xs" title={asked}>
				<span class="font-medium text-muted-foreground">You</span>
				{asked}
			</p>
		{/if}
		{#if said}
			<p class="line-clamp-3 text-sm" title={said}>
				<span class="font-medium text-muted-foreground">Claude</span>
				{said}
			</p>
		{/if}

		{#if steps.length || working}
			<ol class="flex min-w-0 flex-col gap-0.5 border-t pt-2 text-xs">
				{#each steps as step, i (`${step.at}-${i}`)}
					{@const Icon = ICONS[family(step.name)]}
					{@const told = tellStep(step)}
					<li
						class="flex min-w-0 items-baseline gap-1.5 {told.error
							? 'text-critical'
							: 'text-muted-foreground'}"
					>
						<Icon class="size-3 shrink-0 translate-y-0.5" />
						<span class="min-w-0 flex-1 truncate" title={told.error ?? told.detail ?? told.code}
							>{told.text}{#if told.code}{told.text ? ' ' : ''}<span class="font-mono"
									>{told.code}</span
								>{/if}{#if told.error}
								· {told.error}{/if}</span
						>
						{#if known(step.ms) && step.ms >= 1000}
							<span class="shrink-0 tabular-nums">{fmtDuration(step.ms)}</span>
						{/if}
					</li>
				{/each}
				{#if working}
					<li class="flex min-w-0 items-baseline gap-1.5 font-medium">
						{#if Running}<Running class="size-3 shrink-0 translate-y-0.5 text-working" />{:else}
							<i
								class="inline-block size-2 shrink-0 animate-pulse rounded-full bg-working"
								aria-hidden="true"
							></i>
						{/if}
						{#if s.tool}
							<span
								class="min-w-0 flex-1 truncate {s.tool.description ? '' : 'font-mono'}"
								title={toolLabel(s.tool)}>{toolTitle(s.tool)}</span
							>
							<Clock since={s.tool.started_at} class="shrink-0 font-normal" />
						{:else}
							<span class="min-w-0 flex-1 font-normal text-muted-foreground">Thinking</span>
						{/if}
					</li>
				{/if}
			</ol>
		{/if}

		{#if s.interrupting}
			<p class="flex items-center gap-1.5 text-xs font-medium text-warning">
				<OctagonXIcon class="size-3.5 shrink-0" />
				Interrupting: Claude stops at its next step
			</p>
		{/if}
		{#if s.waiting_for && !s.asks.length}
			<p class="truncate font-mono text-xs text-permission">{s.waiting_for}</p>
		{/if}
		{#if s.tasks.length}
			<p class="flex min-w-0 items-center gap-2 text-xs">
				<Meter pct={(100 * tasksDone) / s.tasks.length} level="neutral" label="tasks" />
				<span class="shrink-0 tabular-nums">{tasksDone}/{s.tasks.length}</span>
				{#if activeTask}<span class="min-w-0 truncate text-muted-foreground"
						>{activeTask.active_form || activeTask.subject}</span
					>{/if}
			</p>
		{/if}
		{#if s.subagents.length}<SubagentList subagents={s.subagents} />{/if}
		{#if files.length}
			<p class="truncate text-xs text-muted-foreground" title={files.join('\n')}>
				Edited <span class="font-mono">{files.slice(-FILES).map(base).join(', ')}</span
				>{files.length > FILES ? ` and ${files.length - FILES} more` : ''}
			</p>
		{/if}
		<div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
			{#if known(s.context_pct)}
				<span class="inline-flex items-center gap-1.5"
					><Meter pct={s.context_pct} label="context" /> context {Math.round(s.context_pct)}%</span
				>
			{/if}
			{#if facts.length}<span class="min-w-0">{facts.join(' · ')}</span>{/if}
		</div>
	{/if}
	<Strip session={s} {lane} />
	{#if children}
		<div class="flex flex-col gap-2 border-t pt-2">
			{@render children()}
		</div>
	{/if}
</article>
