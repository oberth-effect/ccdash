<script lang="ts">
	// The rest of the system in a line each: the machines with their link,
	// open issues, the workspace sync and Claude's state, every line a link
	// to its area. What cannot be read (a sync that is off) is left out; a
	// line in the warning colour is one that wants the owner.
	import { Unauthorized } from '#lib/api/client.js';
	import { fetchSync, fetchWorkspace, type SyncView, type WorkspaceView } from '#lib/api/index.js';
	import { fmtDuration, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { DOCS_WORKSPACES } from '#lib/docs.js';
	import { ATTENTION } from '#lib/workspace.js';

	const EVERY_MS = 60_000;

	type Row = { href: string; name: string; text: string; warn: boolean };

	let {
		hostCounts,
		issues
	}: {
		hostCounts: Record<string, number>;
		issues: { open: number; working: number; flagged: number } | null;
	} = $props();

	let workspace = $state<WorkspaceView | null>(null);
	let sync = $state<SyncView | null>(null);

	async function load() {
		// NOTE: only what is on: the hub answers 404 for a sync it does not run
		const [ws, state] = await Promise.allSettled([
			live.hub?.features.workspace ? fetchWorkspace() : null,
			live.anyHost('sync') ? fetchSync() : null
		]);
		if ([ws, state].some((r) => r.status === 'rejected' && r.reason instanceof Unauthorized)) {
			live.needToken = true;
		}
		workspace = ws.status === 'fulfilled' && ws.value?.enabled ? ws.value : null;
		sync = state.status === 'fulfilled' ? state.value : null;
	}

	$effect(() => {
		if (!live.ready) return;
		load();
		const timer = setInterval(load, EVERY_MS);
		return () => clearInterval(timer);
	});

	const units = $derived.by(() => {
		if (!workspace) return null;
		const inStep = workspace.units.filter((u) => {
			const states = Object.values(u.hosts);
			return states.length > 0 && states.every((s) => s.state === 'in_step');
		}).length;
		const attention = workspace.units.filter((u) =>
			Object.values(u.hosts).some((s) => ATTENTION.includes(s.state))
		).length;
		return { inStep, total: workspace.units.length, attention };
	});
	const newDirs = $derived(workspace?.new_dirs.filter((d) => d.state === 'waiting').length ?? 0);

	const rows = $derived(
		[
			live.ready &&
				!live.workspaces.length && {
					href: DOCS_WORKSPACES,
					name: 'Workspaces',
					text: 'none set: the knowledge tools refuse every session',
					warn: true
				},
			issues && {
				href: '/projects',
				name: 'Issues',
				text: `${issues.open} open${issues.working ? `, ${issues.working} in progress` : ''}${
					issues.flagged ? ` · ${plural(issues.flagged, 'note')} flagged` : ''
				}`,
				warn: false
			},
			units && {
				href: '/workspace',
				name: 'Workspace',
				text: units.attention
					? `${plural(units.attention, 'unit')} need${units.attention === 1 ? 's' : ''} you`
					: `${units.inStep} of ${units.total} in step${newDirs ? ` · ${newDirs} new` : ''}`,
				warn: units.attention > 0
			},
			sync && {
				href: '/state',
				name: 'Claude state',
				text: sync.conflicts.length
					? plural(sync.conflicts.length, 'conflict')
					: `${plural(sync.hosts.filter((h) => h.connected).length, 'machine')} in sync`,
				warn: sync.conflicts.length > 0
			}
		].filter((row): row is Row => Boolean(row))
	);
</script>

<ul class="divide-y rounded-lg border bg-card text-sm">
	{#each live.hosts as h (h.name)}
		<li class="flex min-w-0 items-center gap-2 px-3 py-1.5">
			<i
				class="inline-block size-2 shrink-0 rounded-full {h.connected
					? 'bg-your-turn'
					: 'bg-critical'}"
				aria-hidden="true"
			></i>
			<a href="/machines/{encodeURIComponent(h.name)}" class="truncate font-medium hover:underline"
				>{h.name}</a
			>
			<span class="ml-auto shrink-0 text-xs text-muted-foreground tabular-nums"
				>{#if h.connected}{plural(hostCounts[h.name] ?? 0, 'session')}{#if h.rtt_ms !== null}
						· {h.rtt_ms < 1 ? '<1' : h.rtt_ms} ms{/if}{:else}offline for {fmtDuration(
						live.now - h.since
					)}{/if}</span
			>
		</li>
	{/each}
	{#each rows as row (row.href)}
		<li>
			<a
				href={row.href}
				class="flex min-w-0 items-center justify-between gap-2 px-3 py-1.5 hover:bg-accent/40"
			>
				<span class="truncate font-medium">{row.name}</span>
				<span
					class="shrink-0 text-xs {row.warn ? 'font-medium text-warning' : 'text-muted-foreground'}"
					>{row.text}</span
				>
			</a>
		</li>
	{/each}
</ul>
