<script lang="ts">
	// The list's filters in one row: the machines (any number, all by
	// default), the project, how recent, the order. Each menu counts what
	// the other filters leave.
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import XIcon from '@lucide/svelte/icons/x';
	import type { Facets } from '#lib/api/types.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { live } from '#lib/live/store.svelte.js';
	import { projectLabel, SORTS, sortOf, WHEN, type Filters } from '#lib/past.js';
	import { hostColor } from '#lib/workspace.js';

	let {
		filters,
		facets,
		hosts,
		onchange
	}: {
		filters: Filters;
		facets: Facets | null;
		/** Every machine of the list, for stable colours. */
		hosts: string[];
		onchange: (changes: Partial<Filters>) => void;
	} = $props();

	/** The machines to offer: those with sessions, and any picked. */
	const machines = $derived.by(() => {
		const counted = facets?.hosts ?? [];
		const names = [...new Set([...counted.map((h) => h.host), ...filters.hosts])].sort();
		return names.map((host) => ({
			host,
			count: counted.find((h) => h.host === host)?.count ?? 0,
			online: live.hosts.some((h) => h.name === host && h.connected)
		}));
	});
	const machineLabel = $derived(
		!filters.hosts.length
			? 'All machines'
			: filters.hosts.length <= 2
				? filters.hosts.join(' + ')
				: `${filters.hosts.length} machines`
	);

	/** Projects grouped by workspace, the workspace itself first. */
	const groups = $derived.by(() => {
		const out: { ws: string; items: { key: string; label: string; count: number }[] }[] = [];
		const projects = [...(facets?.projects ?? [])];
		if (filters.project && !projects.some((p) => p.key === filters.project))
			projects.push({ key: filters.project, workspace: null, project: null, count: 0 });
		for (const p of projects) {
			const ws = p.key === '-' ? '' : p.key.split('/')[0];
			let group = out.find((g) => g.ws === ws);
			if (!group) out.push((group = { ws, items: [] }));
			const name =
				p.key === '-' ? 'Outside the workspaces' : p.key.split('/')[1] || '(the workspace itself)';
			group.items.push({ key: p.key, label: name, count: p.count });
		}
		for (const g of out)
			g.items.sort((a, b) =>
				a.key.endsWith('/') ? -1 : b.key.endsWith('/') ? 1 : a.label.localeCompare(b.label)
			);
		return out.sort((a, b) => (a.ws === '' ? 1 : b.ws === '' ? -1 : a.ws.localeCompare(b.ws)));
	});
	const projectButton = $derived(filters.project ? projectLabel(filters.project) : 'All projects');
	const whenLabel = $derived(WHEN.find((w) => w.id === filters.when)?.label ?? 'Any time');
	const sort = $derived(sortOf(filters));
	const sortLabel = $derived(SORTS.find((s) => s.id === sort)?.label ?? '');
	const any = $derived(
		Boolean(filters.hosts.length || filters.project || filters.when || filters.sort)
	);

	function toggleHost(host: string, on: boolean) {
		const rest = filters.hosts.filter((h) => h !== host);
		onchange({ hosts: on ? [...rest, host].sort() : rest });
	}
</script>

<div class="flex flex-wrap items-center gap-1.5">
	<DropdownMenu.Root>
		<DropdownMenu.Trigger>
			{#snippet child({ props })}
				<Button
					variant={filters.hosts.length ? 'secondary' : 'outline'}
					size="sm"
					class="max-w-56 gap-1"
					{...props}
				>
					<span class="truncate">{machineLabel}</span>
					<ChevronDownIcon class="size-3 opacity-60" />
				</Button>
			{/snippet}
		</DropdownMenu.Trigger>
		<DropdownMenu.Content align="start" class="min-w-56">
			<DropdownMenu.Label class="text-xs font-normal text-muted-foreground"
				>Sessions any of them ran on</DropdownMenu.Label
			>
			<DropdownMenu.CheckboxItem
				checked={!filters.hosts.length}
				onCheckedChange={() => onchange({ hosts: [] })}>All machines</DropdownMenu.CheckboxItem
			>
			<DropdownMenu.Separator />
			{#each machines as m (m.host)}
				<DropdownMenu.CheckboxItem
					checked={filters.hosts.includes(m.host)}
					closeOnSelect={false}
					onCheckedChange={(on) => toggleHost(m.host, on)}
				>
					<span class="size-2 rounded-full" style="background: {hostColor(hosts, m.host)}"></span>
					<span class="flex-1">{m.host}</span>
					{#if !m.online}<span class="text-xs text-muted-foreground">offline</span>{/if}
					<span class="text-xs text-muted-foreground tabular-nums">{m.count}</span>
				</DropdownMenu.CheckboxItem>
			{/each}
		</DropdownMenu.Content>
	</DropdownMenu.Root>

	<DropdownMenu.Root>
		<DropdownMenu.Trigger>
			{#snippet child({ props })}
				<Button
					variant={filters.project ? 'secondary' : 'outline'}
					size="sm"
					class="max-w-64 gap-1"
					{...props}
				>
					<span class="truncate">{projectButton}</span>
					<ChevronDownIcon class="size-3 opacity-60" />
				</Button>
			{/snippet}
		</DropdownMenu.Trigger>
		<DropdownMenu.Content align="start" class="max-h-[60vh] min-w-60 overflow-y-auto">
			<DropdownMenu.RadioGroup
				value={filters.project}
				onValueChange={(project) => onchange({ project })}
			>
				<DropdownMenu.RadioItem value="">All projects</DropdownMenu.RadioItem>
				{#each groups as g (g.ws)}
					<DropdownMenu.Separator />
					{#if g.ws}<DropdownMenu.Label class="text-xs text-muted-foreground"
							>{g.ws}</DropdownMenu.Label
						>{/if}
					{#each g.items as p (p.key)}
						<DropdownMenu.RadioItem value={p.key}>
							<span class="flex-1 truncate">{p.label}</span>
							<span class="text-xs text-muted-foreground tabular-nums">{p.count}</span>
						</DropdownMenu.RadioItem>
					{/each}
				{/each}
			</DropdownMenu.RadioGroup>
		</DropdownMenu.Content>
	</DropdownMenu.Root>

	<DropdownMenu.Root>
		<DropdownMenu.Trigger>
			{#snippet child({ props })}
				<Button variant={filters.when ? 'secondary' : 'outline'} size="sm" class="gap-1" {...props}>
					{whenLabel}
					<ChevronDownIcon class="size-3 opacity-60" />
				</Button>
			{/snippet}
		</DropdownMenu.Trigger>
		<DropdownMenu.Content align="start">
			<DropdownMenu.RadioGroup value={filters.when} onValueChange={(when) => onchange({ when })}>
				<DropdownMenu.RadioItem value="">Any time</DropdownMenu.RadioItem>
				{#each WHEN as w (w.id)}
					<DropdownMenu.RadioItem value={w.id}>{w.label}</DropdownMenu.RadioItem>
				{/each}
			</DropdownMenu.RadioGroup>
		</DropdownMenu.Content>
	</DropdownMenu.Root>

	<DropdownMenu.Root>
		<DropdownMenu.Trigger>
			{#snippet child({ props })}
				<Button variant="ghost" size="sm" class="gap-1" {...props}>
					<span class="text-muted-foreground">Sort:</span>
					{sortLabel}
					<ChevronDownIcon class="size-3 opacity-60" />
				</Button>
			{/snippet}
		</DropdownMenu.Trigger>
		<DropdownMenu.Content align="start">
			<DropdownMenu.RadioGroup
				value={sort}
				onValueChange={(value) =>
					onchange({
						sort: value === (filters.q.trim() ? 'best' : 'recent') ? '' : (value as Filters['sort'])
					})}
			>
				{#each SORTS as s (s.id)}
					<DropdownMenu.RadioItem value={s.id} disabled={s.needsQuery && !filters.q.trim()}
						>{s.label}</DropdownMenu.RadioItem
					>
				{/each}
			</DropdownMenu.RadioGroup>
		</DropdownMenu.Content>
	</DropdownMenu.Root>

	{#if any}
		<Button
			variant="ghost"
			size="sm"
			class="gap-1 text-muted-foreground"
			onclick={() => onchange({ hosts: [], project: '', when: '', sort: '' })}
		>
			<XIcon /> Clear filters
		</Button>
	{/if}
</div>
