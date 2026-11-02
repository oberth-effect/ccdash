<script lang="ts">
	// Machines: the hub's own machine, then every node (a machine with a
	// collector) the hub knows, one card each with how it is doing, its
	// link, the sessions on it, its copy of Claude's state and its
	// workspace sync, and Wake when it is down; the card opens its page.
	import { Unauthorized } from '#lib/api/client.js';
	import { fetchSync, fetchWorkspace, type SyncView, type WorkspaceView } from '#lib/api/index.js';
	import Power from '#lib/components/machines/Power.svelte';
	import { fmtBytes, fmtDuration, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { basics, sessionLine, share, stateDot, stateLine, systemLine } from '#lib/machines.js';
	import { shown } from '#lib/session.js';
	import { fmtRtt, hostSummary } from '#lib/workspace.js';

	const POLL_MS = 5000;

	let sync = $state<SyncView | null>(null);
	let ws = $state<WorkspaceView | null>(null);

	async function load() {
		const [s, w] = await Promise.allSettled([fetchSync(), fetchWorkspace()]);
		for (const r of [s, w]) {
			if (r.status === 'rejected' && r.reason instanceof Unauthorized) live.needToken = true;
		}
		if (s.status === 'fulfilled') sync = s.value;
		if (w.status === 'fulfilled') ws = w.value;
	}

	$effect(() => {
		if (!live.ready) return;
		load();
		const timer = setInterval(load, POLL_MS);
		return () => clearInterval(timer);
	});

	/** Every host the hub has heard of, connected first. */
	const names = $derived(
		[
			...new Set([
				...live.hosts.map((h) => h.name),
				...(sync?.hosts ?? []).map((h) => h.host),
				...(ws?.hosts ?? []).map((h) => h.host)
			])
		].sort((a, b) => Number(live.connected(b)) - Number(live.connected(a)) || a.localeCompare(b))
	);
	const sessionsOn = (host: string) =>
		live.list.filter((s) => s.host === host && s.activity !== 'gone');
	/** "connected for 6 min · 2 ms", asleep or waking, or how long it has been offline. */
	function linkLine(name: string): string {
		const h = live.hosts.find((x) => x.name === name);
		if (!h) return 'never connected';
		const parts = [stateLine(h, live.now)];
		if (h.connected && h.rtt_ms !== null) parts.push(fmtRtt(h.rtt_ms));
		return parts.join(' · ');
	}
	const hubDisks = $derived(
		(live.hub?.machine?.disks ?? [])
			.map(
				(d) => `${d.path} ${Math.round(share(d.used, d.used + d.avail))} % of ${fmtBytes(d.total)}`
			)
			.join(' · ')
	);
	const needsOn = (host: string) =>
		sessionsOn(host).filter((s) => s.activity === 'permission' || shown(s) === 'your_turn').length;
</script>

<div class="mx-auto flex max-w-6xl flex-col gap-5 p-3 sm:p-4">
	<p class="text-sm text-muted-foreground">
		The hub, and every node: a machine whose collector reports to it, with how it is doing, its
		link, the sessions on it, its copy of Claude's state and its share of the workspace sync. A
		node's card opens its page, with its numbers over the range in the bar and its power.
	</p>

	<section class="flex flex-col gap-2">
		<h2 class="text-base font-semibold">Hub</h2>
		{#if live.hub}
			{@const m = live.hub.machine}
			<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
				<div class="flex flex-col gap-1.5 rounded-lg border bg-card p-3 text-sm">
					<span class="flex items-center gap-2">
						<i class="inline-block size-2.5 rounded-full bg-your-turn" aria-hidden="true"></i>
						<span class="font-semibold">{live.hub.name}</span>
						<span class="ml-auto text-xs text-muted-foreground"
							>ccdash hub up {fmtDuration(live.now - live.hub.started_at)}</span
						>
					</span>
					{#if m}
						<dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs">
							<dt class="text-muted-foreground">Machine</dt>
							<dd>{basics(m, live.now)}</dd>
							<dt class="text-muted-foreground">Disks</dt>
							<dd>{hubDisks}</dd>
							<dt class="text-muted-foreground">System</dt>
							<dd>{systemLine(m)}</dd>
						</dl>
					{:else}
						<p class="text-xs text-muted-foreground">Reading the machine…</p>
					{/if}
				</div>
			</div>
		{/if}
	</section>

	<section class="flex flex-col gap-2">
		<h2 class="text-base font-semibold">Nodes</h2>
		{#if !names.length}
			<p class="text-sm text-muted-foreground">No node has connected yet.</p>
		{:else}
			<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
				{#each names as name (name)}
					{@const h = live.hosts.find((x) => x.name === name)}
					{@const sh = sync?.hosts.find((x) => x.host === name)}
					{@const wh = ws?.hosts.find((x) => x.host === name)}
					{@const on = sessionsOn(name)}
					{@const needs = needsOn(name)}
					<div
						class="relative flex flex-col gap-1.5 rounded-lg border bg-card p-3 text-sm hover:bg-muted/40"
					>
						<span class="flex items-center gap-2">
							<i
								class="inline-block size-2.5 rounded-full {h
									? stateDot(h, live.now)
									: 'bg-critical'}"
								aria-hidden="true"
							></i>
							<!-- NOTE: the link covers the card; the buttons sit above it -->
							<a
								href="/machines/{encodeURIComponent(name)}"
								class="font-semibold after:absolute after:inset-0 after:content-['']">{name}</a
							>
							<span class="ml-auto text-xs text-muted-foreground">{linkLine(name)}</span>
						</span>
						<dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs">
							{#if h?.machine}
								<dt class="text-muted-foreground">Machine</dt>
								<dd>
									{h.connected ? basics(h.machine, live.now) : systemLine(h.machine)}
								</dd>
							{/if}
							{#if h && sessionLine(h)}
								<dt class="text-muted-foreground">Screen</dt>
								<dd class={sessionLine(h) === 'unlocked' ? 'text-permission' : ''}>
									{sessionLine(h)}
								</dd>
							{/if}
							<dt class="text-muted-foreground">Sessions</dt>
							<dd>
								{plural(on.length, 'session')}{#if needs}, <span class="text-permission"
										>{needs} need you</span
									>{/if}
							</dd>
							<dt class="text-muted-foreground">Claude state</dt>
							<dd>
								{#if sh?.status}
									{plural(sh.status.files, 'file')}, {fmtBytes(
										sh.status.bytes
									)}{#if sh.status.pending_up || sh.status.pending_down}
										· {sh.status.pending_up + sh.status.pending_down} moving{:else}
										· in step{/if}
								{:else}
									not syncing
								{/if}
							</dd>
							<dt class="text-muted-foreground">Workspace</dt>
							<dd>
								{#if wh}
									{hostSummary(wh, live.now)}
								{:else}
									not syncing
								{/if}
							</dd>
							{#if wh?.status?.cost}
								<dt class="text-muted-foreground">Collector</dt>
								<dd>
									{fmtBytes(wh.status.cost.rss_kb * 1024)} resident · {(
										(wh.status.cost.cpu_ms + wh.status.cost.git_cpu_ms) /
										1000
									).toFixed(0)} s CPU since start
								</dd>
							{/if}
						</dl>
						{#if h}<Power host={h} compact />{/if}
					</div>
				{/each}
			</div>
		{/if}
	</section>
</div>
