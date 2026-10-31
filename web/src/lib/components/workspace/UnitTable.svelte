<script lang="ts">
	// Workspace › Units: every unit as a row that sorts by its columns,
	// with a search, state filters and one compact chip per machine (a
	// full state chip when the bar's host filter picks one machine).
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
	import type { UnitState, UnitStatus, UnitView, WorkspaceHost } from '#lib/api/types.js';
	import StateChip from '#lib/components/workspace/StateChip.svelte';
	import { Input } from '#lib/components/ui/input/index.js';
	import { fmtBytes, fmtStamp, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { cn } from '#lib/utils.js';
	import { ATTENTION, STATE, bundleOf, unitHref, unitName, worst } from '#lib/workspace.js';

	let { units, hosts }: { units: UnitView[]; hosts: WorkspaceHost[] } = $props();

	type Filter =
		| 'all'
		| 'attention'
		| 'conflicts'
		| 'held'
		| 'failing'
		| 'adopt'
		| 'moving'
		| 'paused'
		| 'today';
	const FILTERS: { id: Filter; label: string }[] = [
		{ id: 'all', label: 'All' },
		{ id: 'attention', label: 'Needs you' },
		{ id: 'conflicts', label: 'Conflicts' },
		{ id: 'held', label: 'Held' },
		{ id: 'failing', label: 'Failing' },
		{ id: 'adopt', label: 'To adopt' },
		{ id: 'moving', label: 'Moving' },
		{ id: 'paused', label: 'Paused' },
		{ id: 'today', label: 'Changed today' }
	];
	type SortKey = 'unit' | 'kind' | 'state' | 'size' | 'version' | 'at';
	const COLUMNS: { key: SortKey; label: string; right?: boolean }[] = [
		{ key: 'unit', label: 'Unit' },
		{ key: 'kind', label: 'Kind' },
		{ key: 'state', label: 'State' },
		{ key: 'size', label: 'Size', right: true },
		{ key: 'version', label: 'Pushes', right: true },
		{ key: 'at', label: 'Last push', right: true }
	];
	/** States in the order the State column sorts them: what needs you first. */
	const ORDER: UnitState[] = [
		'conflict',
		'held',
		'failed',
		'unadopted',
		'seeding',
		'creating',
		'merging',
		'ahead',
		'behind',
		'skipped',
		'waiting',
		'missing',
		'paused',
		'in_step'
	];
	const MOVING: UnitState[] = ['ahead', 'behind', 'merging', 'seeding', 'creating'];
	const DAY_MS = 86_400_000;

	let filter = $state<Filter>('all');
	let query = $state('');
	let sortKey = $state<SortKey>('unit');
	let desc = $state(false);

	const hostNames = $derived(hosts.map((h) => h.host));
	/** The one machine the bar picked, when it did. */
	const only = $derived(hostNames.length === 1 ? hostNames[0] : null);

	type Row = {
		u: UnitView;
		name: string;
		bundle: string;
		worst: UnitState | null;
		states: { host: string; s: UnitStatus | undefined }[];
		haystack: string;
	};
	const rows = $derived<Row[]>(
		units.map((u) => {
			const states = hostNames.map((host) => ({ host, s: u.hosts[host] }));
			const narrowed: UnitView = {
				...u,
				hosts: Object.fromEntries(states.filter((x) => x.s).map((x) => [x.host, x.s!]))
			};
			return {
				u,
				name: unitName(u.unit),
				bundle: bundleOf(u.unit),
				worst: worst(narrowed),
				states,
				haystack: [
					u.unit,
					u.kind,
					...states.flatMap((x) => (x.s ? [x.s.state, x.s.detail ?? ''] : []))
				]
					.join(' ')
					.toLowerCase()
			};
		})
	);

	function passes(r: Row): boolean {
		const q = query.trim().toLowerCase();
		if (q && !q.split(/\s+/).every((word) => r.haystack.includes(word))) return false;
		const states = r.states.map((x) => x.s?.state).filter((s): s is UnitState => !!s);
		switch (filter) {
			case 'attention':
				return states.some((s) => ATTENTION.includes(s));
			case 'conflicts':
				return r.states.some((x) => (x.s?.conflicts.length ?? 0) > 0);
			case 'held':
				return r.states.some((x) => x.s?.held != null);
			case 'failing':
				return states.includes('failed');
			case 'adopt':
				return states.includes('unadopted');
			case 'moving':
				return states.some((s) => MOVING.includes(s));
			case 'paused':
				return r.u.paused;
			case 'today':
				return live.now - r.u.at < DAY_MS;
			default:
				return true;
		}
	}

	const shown = $derived.by(() => {
		const list = rows.filter(passes);
		const dir = desc ? -1 : 1;
		const rank = (s: UnitState | null) => (s ? ORDER.indexOf(s) : ORDER.length);
		list.sort((a, b) => {
			let c = 0;
			switch (sortKey) {
				case 'unit':
					c = a.u.unit.localeCompare(b.u.unit);
					break;
				case 'kind':
					c = a.u.kind.localeCompare(b.u.kind) || a.u.unit.localeCompare(b.u.unit);
					break;
				case 'state':
					c = rank(a.worst) - rank(b.worst) || a.u.unit.localeCompare(b.u.unit);
					break;
				case 'size':
					c = a.u.size - b.u.size;
					break;
				case 'version':
					c = a.u.version - b.u.version;
					break;
				case 'at':
					c = a.u.at - b.u.at;
					break;
			}
			return c * dir;
		});
		return list;
	});

	function sortBy(key: SortKey) {
		if (sortKey === key) desc = !desc;
		else {
			sortKey = key;
			// numbers and times read best largest first; names and states as listed
			desc = key === 'size' || key === 'version' || key === 'at';
		}
	}

	/** The chip's dot for a machine's state. */
	const dot = (s: UnitStatus | undefined) => (s ? STATE[s.state].dot : 'bg-idle opacity-50');
	const chipTitle = (host: string, s: UnitStatus | undefined) =>
		s
			? `${host}: ${STATE[s.state].label}${s.detail ? ` · ${s.detail}` : ''}`
			: `${host}: has not reported this unit`;
</script>

<section class="flex flex-col gap-2">
	<div class="flex flex-wrap items-center gap-2">
		<h2 class="text-base font-semibold">Units</h2>
		<span class="text-xs text-muted-foreground">{shown.length} of {units.length}</span>
		<Input
			bind:value={query}
			class="h-7 max-w-56 text-xs"
			placeholder="Search units, states, details"
			aria-label="Search units"
		/>
		<div class="ml-auto flex flex-wrap items-center gap-1">
			{#each FILTERS as f (f.id)}
				<button
					type="button"
					class={cn(
						'rounded-full border px-2 py-0.5 text-xs',
						filter === f.id && 'bg-sidebar-accent font-medium'
					)}
					aria-pressed={filter === f.id}
					onclick={() => (filter = f.id)}>{f.label}</button
				>
			{/each}
		</div>
	</div>
	{#if !units.length}
		<p class="text-sm text-muted-foreground">
			No unit travels in this scope. Collectors report what they find at unit depth; include
			directories under Configuration, or widen the globs.
		</p>
	{:else}
		<div class="overflow-x-auto rounded-md border">
			<table class="w-full text-sm">
				<thead class="bg-muted/50 text-left text-xs text-muted-foreground">
					<tr>
						{#each COLUMNS as c (c.key)}
							<th
								class={cn('px-2 py-1.5 font-medium', c.right && 'text-right')}
								aria-sort={sortKey === c.key ? (desc ? 'descending' : 'ascending') : undefined}
							>
								<button
									type="button"
									class="inline-flex items-center gap-1 hover:text-foreground"
									onclick={() => sortBy(c.key)}
								>
									{c.label}
									{#if sortKey === c.key}
										{#if desc}<ArrowDownIcon class="size-3" />{:else}<ArrowUpIcon
												class="size-3"
											/>{/if}
									{/if}
								</button>
							</th>
							{#if c.key === 'state' && !only}
								<th class="px-2 py-1.5 font-medium">Machines</th>
							{/if}
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each shown as r (r.u.unit)}
						<tr class="border-t hover:bg-muted/40">
							<td class="px-2 py-1">
								<a href={unitHref(r.u.unit)} class="flex items-baseline gap-1 hover:underline">
									<span class="text-xs text-muted-foreground">{r.bundle}/</span>
									<span class="font-mono text-xs">{r.name}</span>
									{#if r.u.paused}
										<span class="rounded bg-muted px-1 text-[10px] text-muted-foreground"
											>paused</span
										>
									{/if}
								</a>
							</td>
							<td class="px-2 py-1 text-xs text-muted-foreground">{r.u.kind}</td>
							<td class="px-2 py-1">
								{#if only}
									{@const s = r.u.hosts[only]}
									<StateChip
										state={s?.state ?? null}
										detail={s?.detail ?? (s ? null : 'this host has not reported the unit')}
									/>
								{:else}
									<StateChip state={r.worst} />
								{/if}
							</td>
							{#if !only}
								<td class="px-2 py-1">
									<div class="flex flex-wrap gap-1">
										{#each r.states as x (x.host)}
											<span
												class="inline-flex max-w-24 items-center gap-1 rounded-full border px-1.5 py-0.5 text-[10px] text-muted-foreground"
												title={chipTitle(x.host, x.s)}
											>
												<i
													class={cn(
														'inline-block size-1.5 shrink-0 rounded-full',
														dot(x.s),
														x.s && STATE[x.s.state].busy && 'animate-pulse'
													)}
													aria-hidden="true"
												></i>
												<span class="truncate">{x.host}</span>
											</span>
										{/each}
									</div>
								</td>
							{/if}
							<td class="px-2 py-1 text-right text-xs text-muted-foreground tabular-nums"
								>{r.u.size ? fmtBytes(r.u.size) : '—'}</td
							>
							<td class="px-2 py-1 text-right text-xs text-muted-foreground tabular-nums"
								>{r.u.version}</td
							>
							<td class="px-2 py-1 text-right text-xs text-muted-foreground tabular-nums">
								{r.u.version ? fmtStamp(r.u.at) : '—'}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		{#if shown.length !== units.length}
			<p class="text-xs text-muted-foreground">
				{plural(units.length - shown.length, 'unit')} hidden by the filter.
			</p>
		{/if}
	{/if}
</section>
