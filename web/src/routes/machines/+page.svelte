<script lang="ts">
	// Machines: every host the hub knows, one card each with its link, the
	// sessions on it, its copy of Claude's state and its workspace sync;
	// the card opens the machine's page.
	import { Unauthorized } from '#lib/api/client.js';
	import { fetchSync, fetchWorkspace, type SyncView, type WorkspaceView } from '#lib/api/index.js';
	import { fmtBytes, fmtDuration, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
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
	/** "connected for 6 min · 2 ms", or how long it has been offline. */
	function linkLine(name: string): string {
		const h = live.hosts.find((x) => x.name === name);
		if (!h) return 'never connected since the hub started';
		const parts = [
			`${h.connected ? 'connected' : 'offline'} for ${fmtDuration(live.now - h.since)}`
		];
		if (h.connected && h.rtt_ms !== null) parts.push(fmtRtt(h.rtt_ms));
		return parts.join(' · ');
	}
	const needsOn = (host: string) =>
		sessionsOn(host).filter((s) => s.activity === 'permission' || shown(s) === 'your_turn').length;
</script>

<div class="mx-auto flex max-w-6xl flex-col gap-4 p-3 sm:p-4">
	<p class="text-sm text-muted-foreground">
		Every machine with a collector: its link to the hub, the sessions on it, its copy of Claude's
		state and its share of the workspace sync. A card opens the machine's page with its numbers over
		the range in the bar.
	</p>
	{#if !names.length}
		<p class="text-sm text-muted-foreground">No machine has connected yet.</p>
	{:else}
		<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
			{#each names as name (name)}
				{@const h = live.hosts.find((x) => x.name === name)}
				{@const sh = sync?.hosts.find((x) => x.host === name)}
				{@const wh = ws?.hosts.find((x) => x.host === name)}
				{@const on = sessionsOn(name)}
				{@const needs = needsOn(name)}
				<a
					href="/machines/{encodeURIComponent(name)}"
					class="flex flex-col gap-1.5 rounded-lg border bg-card p-3 text-sm hover:bg-muted/40"
				>
					<span class="flex items-center gap-2">
						<i
							class="inline-block size-2.5 rounded-full {h?.connected
								? 'bg-your-turn'
								: 'bg-critical'}"
							aria-hidden="true"
						></i>
						<span class="font-semibold">{name}</span>
						<span class="ml-auto text-xs text-muted-foreground">{linkLine(name)}</span>
					</span>
					<dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs">
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
				</a>
			{/each}
		</div>
	{/if}
</div>
