<script lang="ts">
	// Workspace › Configuration: the rules with what each one does (globs
	// with their match counts, the never-travelling names with what they
	// keep home), the limits, the conflict shape, the per-bundle thresholds;
	// then Coverage: what every machine's root holds, what travels, what
	// stays home and why. A draft is previewed by the hub as you type.
	import PlusIcon from '@lucide/svelte/icons/plus';
	import XIcon from '@lucide/svelte/icons/x';
	import { Unauthorized } from '#lib/api/client.js';
	import {
		fetchWorkspaceCoverage,
		previewWorkspaceConfig,
		putWorkspaceConfig,
		workspaceUnitOp,
		type Coverage,
		type CoverageUnit,
		type Ignored,
		type Preview,
		type WorkspaceConfig,
		type WorkspaceView
	} from '#lib/api/index.js';
	import StatTile from '#lib/components/charts/StatTile.svelte';
	import Tiles from '#lib/components/charts/Tiles.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { fmtBytes, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { scope } from '#lib/scope.svelte.js';
	import { cn } from '#lib/utils.js';
	import { bundleOf, COVERAGE_STATUS, decisionText, unitHref } from '#lib/workspace.js';

	let { view, onchange }: { view: WorkspaceView; onchange: () => void } = $props();

	const PREVIEW_DEBOUNCE_MS = 400;
	const COVERAGE_EVERY_MS = 60_000;

	// the draft
	let include = $state<string[]>([]);
	let exclude = $state<string[]>([]);
	let always = $state<string[]>([]);
	let ignored = $state<Ignored[]>([]);
	let depth = $state(2);
	let maxFile = $state(64);
	let autoInclude = $state(500);
	let conflicts = $state<'markers' | 'set_aside'>('markers');
	let perBundle = $state<Record<string, string>>({});
	let loaded = $state(-1);
	let dirty = $state(false);
	let busy = $state(false);
	let error = $state('');
	let saved = $state(false);

	let preview = $state<Preview | null>(null);
	let previewError = $state('');
	let coverage = $state<Coverage | null>(null);
	let leftOutFilter = $state<'all' | 'left_out' | 'waiting' | 'ignored' | 'unsyncable'>('all');
	let acting = $state<string | null>(null);

	const bundles = $derived(
		[
			...new Set([...view.units.map((u) => bundleOf(u.unit)), ...Object.keys(view.config.bundles)])
		].sort()
	);

	// NOTE: the form follows the hub's version only while nothing is typed
	$effect(() => {
		if (loaded === view.config_version || dirty) return;
		loaded = view.config_version;
		const c = view.config;
		include = [...c.include];
		exclude = [...c.exclude];
		always = [...c.always_exclude];
		ignored = c.sync_ignored.map((i) => ({ ...i }));
		depth = c.depth;
		maxFile = c.max_file_mb;
		autoInclude = c.auto_include_mb;
		conflicts = c.conflicts;
		perBundle = Object.fromEntries(
			bundles.map((b) => [b, c.bundles[b]?.auto_include_mb?.toString() ?? ''])
		);
	});

	const clean = (list: string[]) => list.map((l) => l.trim()).filter(Boolean);

	/** The draft as the hub takes it. */
	function draft(): WorkspaceConfig {
		const next: WorkspaceConfig = {
			...view.config,
			depth: Math.min(6, Math.max(1, Math.round(depth) || 2)),
			include: clean(include),
			exclude: clean(exclude),
			always_exclude: clean(always),
			sync_ignored: ignored
				.map((i) => ({ pattern: i.pattern.trim(), quiescent: i.quiescent }))
				.filter((i) => i.pattern),
			max_file_mb: Math.max(1, Math.round(maxFile) || 64),
			auto_include_mb: Math.max(0, Math.round(autoInclude) || 0),
			conflicts,
			bundles: Object.fromEntries(
				Object.entries(view.config.bundles).map(([b, o]) => [b, { ...o }])
			)
		};
		for (const [b, text] of Object.entries(perBundle)) {
			const value = text.trim() === '' ? null : Number(text);
			const current = next.bundles[b] ?? {
				auto_include_mb: null,
				sync_ignored: [],
				always_exclude: []
			};
			current.auto_include_mb = value === null || Number.isNaN(value) ? null : value;
			if (
				current.auto_include_mb === null &&
				!current.sync_ignored.length &&
				!current.always_exclude.length
			)
				delete next.bundles[b];
			else next.bundles[b] = current;
		}
		return next;
	}

	// the hub previews the draft as it changes
	let previewTimer: ReturnType<typeof setTimeout> | null = null;
	let previewSeq = 0;
	function touched() {
		dirty = true;
		saved = false;
		if (previewTimer) clearTimeout(previewTimer);
		previewTimer = setTimeout(loadPreview, PREVIEW_DEBOUNCE_MS);
	}
	async function loadPreview() {
		const seq = ++previewSeq;
		try {
			const next = await previewWorkspaceConfig(draft());
			if (seq === previewSeq) {
				preview = next;
				previewError = '';
			}
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else previewError = (e as Error).message;
		}
	}

	async function loadCoverage() {
		try {
			coverage = await fetchWorkspaceCoverage();
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		}
	}

	$effect(() => {
		if (!live.ready) return;
		loadCoverage();
		loadPreview();
		const timer = setInterval(loadCoverage, COVERAGE_EVERY_MS);
		return () => clearInterval(timer);
	});

	async function save() {
		busy = true;
		error = '';
		saved = false;
		try {
			await putWorkspaceConfig(draft());
			saved = true;
			dirty = false;
			onchange();
			loadCoverage();
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}

	function discard() {
		dirty = false;
		loaded = -1;
		saved = false;
		loadPreview();
	}

	async function act(unit: string, op: 'include' | 'ignore') {
		acting = unit;
		error = '';
		try {
			await workspaceUnitOp(unit, { op, host: null, value: null });
			onchange();
			loadCoverage();
			loadPreview();
		} catch (e) {
			error = (e as Error).message;
		} finally {
			acting = null;
		}
	}

	/** The preview's match count of a glob. */
	const matches = (pattern: string, exclude: boolean) =>
		preview?.globs.find((g) => g.pattern === pattern.trim() && g.exclude === exclude)?.matches ??
		null;

	/** What a never-travelling name keeps home, across the workspace. */
	const keepsHome = (name: string) => coverage?.names.find((n) => n.name === name.trim()) ?? null;

	/** Whether the draft lets a unit travel. */
	const draftTravels = (u: CoverageUnit) =>
		u.status !== 'unsyncable' &&
		u.status !== 'ignored' &&
		(u.decision.kind === 'selected' || u.decision.kind === 'forced_in');

	/** The current decisions, with the draft's where it differs; narrowed to
	 * the bar's workspaces. */
	const units = $derived.by(() => {
		if (!coverage) return [];
		const { ws } = scope.current;
		const drafted = Object.fromEntries((preview?.units ?? []).map((u) => [u.unit, u]));
		return coverage.units
			.filter((u) => !ws.length || ws.includes(bundleOf(u.unit)))
			.map((u) => {
				const d = drafted[u.unit];
				const now = u.status === 'travels';
				const then = d ? draftTravels(d) : now;
				return { ...u, draft: d ?? null, change: dirty && then !== now ? then : null };
			});
	});
	const notTravelling = $derived(
		units
			.filter((u) => u.status !== 'travels')
			.filter((u) => leftOutFilter === 'all' || u.status === leftOutFilter)
			.sort((a, b) => b.size + sum(b.excluded) - a.size - sum(a.excluded))
	);
	const sum = (m: Record<string, number>) => Object.values(m).reduce((n, v) => n + v, 0);
	const travelling = $derived(units.filter((u) => u.status === 'travels'));
	const leftOutBytes = $derived(
		units.filter((u) => u.status !== 'travels').reduce((n, u) => n + u.size + sum(u.excluded), 0)
	);
	const homeBytes = $derived(coverage?.names.reduce((n, x) => n + x.bytes, 0) ?? 0);
	const changes = $derived(units.filter((u) => u.change !== null));
	const FILTERS = [
		['all', 'All'],
		['left_out', 'Left out by the globs'],
		['waiting', 'Waiting'],
		['ignored', 'Ignored'],
		['unsyncable', 'Cannot travel']
	] as const;
</script>

{#snippet globRows(list: string[], exclude: boolean, placeholder: string)}
	<ul class="flex flex-col gap-1">
		{#each list as row, i (i)}
			{@const n = matches(row, exclude)}
			<li class="flex items-center gap-1.5">
				<Input
					bind:value={list[i]}
					oninput={touched}
					class="h-7 font-mono text-xs"
					{placeholder}
					aria-label={exclude ? 'Exclude glob' : 'Include glob'}
				/>
				<span
					class={cn(
						'w-24 shrink-0 text-right text-[11px] tabular-nums',
						n === 0 ? 'text-permission' : 'text-muted-foreground'
					)}
				>
					{#if row.trim() === ''}
						&nbsp;
					{:else if n === null}
						…
					{:else if n === 0}
						matches nothing
					{:else}
						{plural(n, 'unit')}
					{/if}
				</span>
				<Button
					variant="ghost"
					size="icon-xs"
					aria-label="Remove"
					onclick={() => {
						list.splice(i, 1);
						touched();
					}}><XIcon /></Button
				>
			</li>
		{/each}
	</ul>
	<Button
		variant="ghost"
		size="xs"
		class="self-start"
		onclick={() => {
			list.push('');
			touched();
		}}><PlusIcon /> Add</Button
	>
{/snippet}

<div class="flex flex-col gap-5">
	<form
		class="flex flex-col gap-4 rounded-lg border bg-card p-3"
		onsubmit={(e) => {
			e.preventDefault();
			save();
		}}
	>
		<div class="flex flex-wrap items-baseline gap-2">
			<h2 class="text-base font-semibold">Rules</h2>
			<span class="text-xs text-muted-foreground"
				>revision {Math.floor(view.config_version / 1_000_000)}, on every collector</span
			>
			{#if previewError}<span class="text-xs text-critical">{previewError}</span>{/if}
		</div>
		<div class="grid gap-4 sm:grid-cols-2">
			<div class="flex flex-col gap-1.5 text-xs">
				<span class="font-medium">Include</span>
				<span class="text-muted-foreground"
					>Globs on unit ids (<code class="font-mono">bundle/repo</code>;
					<code class="font-mono">*</code>
					spans <code class="font-mono">/</code>); none means every directory.</span
				>
				{@render globRows(include, false, 'bundle/*')}
			</div>
			<div class="flex flex-col gap-1.5 text-xs">
				<span class="font-medium">Exclude</span>
				<span class="text-muted-foreground">Wins over include.</span>
				{@render globRows(exclude, true, 'bundle/repo or *_bkp')}
			</div>
			<div class="flex flex-col gap-1.5 text-xs">
				<span class="font-medium">Never travels</span>
				<span class="text-muted-foreground"
					>Directory names, wherever they appear; what each keeps home is measured every six hours.</span
				>
				<ul class="flex flex-col gap-1">
					{#each always as name, i (i)}
						{@const k = keepsHome(name)}
						<li class="flex items-center gap-1.5">
							<Input
								bind:value={always[i]}
								oninput={touched}
								class="h-7 font-mono text-xs"
								placeholder="node_modules"
								aria-label="Never-travelling directory name"
							/>
							<span class="w-32 shrink-0 text-right text-[11px] text-muted-foreground tabular-nums">
								{#if k}{fmtBytes(k.bytes)} in {plural(
										k.units,
										'unit'
									)}{:else if always[i].trim()}nothing found{/if}
							</span>
							<Button
								variant="ghost"
								size="icon-xs"
								aria-label="Remove"
								onclick={() => {
									always.splice(i, 1);
									touched();
								}}><XIcon /></Button
							>
						</li>
					{/each}
				</ul>
				<Button
					variant="ghost"
					size="xs"
					class="self-start"
					onclick={() => {
						always.push('');
						touched();
					}}><PlusIcon /> Add</Button
				>
			</div>
			<div class="flex flex-col gap-1.5 text-xs">
				<span class="font-medium">Travels although ignored</span>
				<span class="text-muted-foreground"
					>Gitignored paths carried anyway; a quiescent one moves only while no local process holds
					it open (state directories under <code class="font-mono">.devenv/state</code> always are).</span
				>
				<ul class="flex flex-col gap-1">
					{#each ignored as entry, i (i)}
						<li class="flex items-center gap-1.5">
							<Input
								bind:value={entry.pattern}
								oninput={touched}
								class="h-7 font-mono text-xs"
								placeholder=".env"
								aria-label="Ignored path that travels"
							/>
							<label class="flex shrink-0 items-center gap-1 text-[11px] text-muted-foreground">
								<Checkbox
									bind:checked={entry.quiescent}
									onCheckedChange={touched}
									aria-label="Quiescent"
								/>
								quiescent
							</label>
							<Button
								variant="ghost"
								size="icon-xs"
								aria-label="Remove"
								onclick={() => {
									ignored.splice(i, 1);
									touched();
								}}><XIcon /></Button
							>
						</li>
					{/each}
				</ul>
				<Button
					variant="ghost"
					size="xs"
					class="self-start"
					onclick={() => {
						ignored.push({ pattern: '', quiescent: false });
						touched();
					}}><PlusIcon /> Add</Button
				>
			</div>
		</div>
		<div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
			<label class="flex flex-col gap-1 text-xs">
				<span class="font-medium">Unit depth</span>
				<Input type="number" min={1} max={6} bind:value={depth} oninput={touched} class="h-8" />
			</label>
			<label class="flex flex-col gap-1 text-xs">
				<span class="font-medium">Skip untracked files above (MB)</span>
				<Input type="number" min={1} bind:value={maxFile} oninput={touched} class="h-8" />
			</label>
			<label class="flex flex-col gap-1 text-xs">
				<span class="font-medium">Include new directories under (MB)</span>
				<Input type="number" min={0} bind:value={autoInclude} oninput={touched} class="h-8" />
			</label>
			<label class="flex flex-col gap-1 text-xs">
				<span class="font-medium">Text conflicts</span>
				<select
					bind:value={conflicts}
					onchange={touched}
					class="h-8 rounded-md border bg-background px-2 text-xs"
				>
					<option value="markers">markers in the file</option>
					<option value="set_aside">keep local, other side on the hub</option>
				</select>
			</label>
		</div>
		{#if bundles.length}
			<div class="flex flex-col gap-1 text-xs">
				<span class="font-medium">Auto-include threshold per bundle (MB, empty = default)</span>
				<div class="flex flex-wrap gap-2">
					{#each bundles as b (b)}
						<label class="flex items-center gap-1">
							<span class="font-mono">{b}</span>
							<Input
								bind:value={perBundle[b]}
								oninput={touched}
								class="h-7 w-20 text-xs"
								placeholder={String(autoInclude)}
							/>
						</label>
					{/each}
				</div>
			</div>
		{/if}
		<div class="flex flex-wrap items-center gap-2">
			<Button type="submit" size="sm" disabled={busy || !dirty}>Save</Button>
			{#if dirty}
				<Button type="button" variant="ghost" size="sm" disabled={busy} onclick={discard}
					>Discard</Button
				>
				{#if changes.length}
					<span class="text-xs text-muted-foreground">
						The draft would change {plural(changes.length, 'unit')}: {changes.filter(
							(u) => u.change === true
						).length} start travelling, {changes.filter((u) => u.change === false).length} stop.
					</span>
				{/if}
			{/if}
			{#if saved}<span class="text-xs text-muted-foreground" role="status"
					>Saved; every collector has it.</span
				>{/if}
			{#if error}<span class="text-sm text-critical" role="alert">{error}</span>{/if}
		</div>
	</form>

	<section class="flex flex-col gap-3">
		<h2 class="text-base font-semibold">Coverage</h2>
		{#if !coverage}
			<p class="text-sm text-muted-foreground">Loading…</p>
		{:else}
			<Tiles>
				<StatTile
					name="Travels"
					value={fmtBytes(travelling.reduce((n, u) => n + u.size, 0))}
					sub="{plural(travelling.length, 'unit')} · {fmtBytes(coverage.hub_bytes)} on the hub"
				/>
				<StatTile
					name="Stays home: inside units"
					value={fmtBytes(homeBytes)}
					sub="the never-travelling names, every directory counted"
				/>
				<StatTile
					name="Stays home: whole directories"
					value={fmtBytes(leftOutBytes)}
					sub="{plural(units.length - travelling.length, 'directory', 'directories')} at unit depth"
				/>
				{#each coverage.hosts as h (h.host)}
					<StatTile
						name={h.host}
						value={fmtBytes(h.bytes)}
						sub="in the root · {fmtBytes(h.travels)} travels, {fmtBytes(h.bytes - h.travels)} stays"
					/>
				{/each}
			</Tiles>
			<p class="text-xs text-muted-foreground">
				Sizes are what the collectors measured on disk (the largest copy when the machines differ);
				the hub holds packed history, so it weighs less than the working trees it carries. Measured
				at start and every six hours.
			</p>

			{#if coverage.names.length}
				<div class="rounded-lg border bg-card p-3">
					<div class="mb-2 text-sm font-medium">Never travels, by name</div>
					<div class="overflow-x-auto">
						<table class="w-full text-xs tabular-nums">
							<thead class="text-left text-muted-foreground">
								<tr>
									<th class="py-1 pr-2 font-medium">Directory name</th>
									<th class="py-1 pr-2 text-right font-medium">Stays home</th>
									<th class="py-1 text-right font-medium">In units</th>
								</tr>
							</thead>
							<tbody>
								{#each coverage.names as n (n.name)}
									<tr class="border-t">
										<td class="py-1 pr-2 font-mono">{n.name}</td>
										<td class="py-1 pr-2 text-right">{fmtBytes(n.bytes)}</td>
										<td class="py-1 text-right">{n.units}</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				</div>
			{/if}

			<div class="flex flex-col gap-2 rounded-lg border bg-card p-3">
				<div class="flex flex-wrap items-center gap-2">
					<div class="text-sm font-medium">Not travelling</div>
					<span class="text-xs text-muted-foreground">{notTravelling.length} directories</span>
					<div class="ml-auto flex flex-wrap gap-1">
						{#each FILTERS as [id, label] (id)}
							<button
								type="button"
								class={cn(
									'rounded-full border px-2 py-0.5 text-xs',
									leftOutFilter === id && 'bg-sidebar-accent font-medium'
								)}
								aria-pressed={leftOutFilter === id}
								onclick={() => (leftOutFilter = id)}>{label}</button
							>
						{/each}
					</div>
				</div>
				{#if !notTravelling.length}
					<p class="text-xs text-muted-foreground">Everything the machines see travels.</p>
				{:else}
					<ul class="flex flex-col divide-y text-sm">
						{#each notTravelling as u (u.unit)}
							<li class="flex flex-wrap items-center gap-x-3 gap-y-0.5 py-1.5">
								<div class="flex min-w-0 flex-1 flex-col">
									<span class="truncate font-mono text-xs">{u.unit}</span>
									<span class="text-xs text-muted-foreground">
										{u.kind} · {fmtBytes(u.size)}{#if sum(u.excluded)}
											+ {fmtBytes(sum(u.excluded))} never travels{/if} · on {u.hosts.join(', ')} ·
										{COVERAGE_STATUS[u.status] ?? u.status}{#if u.reason}: {u.reason}{:else if u.status === 'left_out'}:
											{decisionText(u.decision)}{/if}
										{#if u.change === true}
											<span class="text-your-turn">→ would travel with the draft</span>
										{/if}
									</span>
								</div>
								{#if u.status !== 'unsyncable'}
									<Button
										size="xs"
										variant={u.status === 'waiting' ? 'default' : 'outline'}
										disabled={acting !== null}
										onclick={() => act(u.unit, 'include')}>Include</Button
									>
								{/if}
								{#if u.status === 'waiting'}
									<Button
										variant="ghost"
										size="xs"
										disabled={acting !== null}
										onclick={() => act(u.unit, 'ignore')}>Ignore</Button
									>
								{/if}
							</li>
						{/each}
					</ul>
				{/if}
			</div>

			{#if changes.some((u) => u.change === false)}
				<div class="rounded-lg border bg-card p-3 text-xs">
					<div class="mb-1 text-sm font-medium">The draft would stop</div>
					<ul class="flex flex-wrap gap-x-3 gap-y-0.5 font-mono">
						{#each changes.filter((u) => u.change === false) as u (u.unit)}
							<li>
								{u.unit}
								<span class="font-sans text-muted-foreground"
									>({decisionText(u.draft?.decision ?? u.decision)})</span
								>
							</li>
						{/each}
					</ul>
				</div>
			{/if}

			{#if coverage.home.length}
				<div class="rounded-lg border bg-card p-3">
					<div class="mb-1 text-sm font-medium">Gitignored inside travelling units</div>
					<p class="mb-2 text-xs text-muted-foreground">
						What the collectors report as ignored directories in units that travel, largest first; a
						unit's page has the switch to carry one.
					</p>
					<ul class="flex flex-col divide-y text-xs">
						{#each coverage.home.slice(0, 25) as d (`${d.unit}/${d.path}`)}
							<li class="flex flex-wrap items-center gap-x-3 py-1">
								<a href={unitHref(d.unit)} class="font-mono hover:underline">{d.unit}</a>
								<span class="font-mono text-muted-foreground">{d.path}</span>
								<span class="ml-auto text-muted-foreground tabular-nums"
									>{fmtBytes(d.bytes)}, {plural(d.files, 'file')} · {d.carried
										? 'carried'
										: 'stays home'}</span
								>
							</li>
						{/each}
					</ul>
				</div>
			{/if}
		{/if}
	</section>
</div>
