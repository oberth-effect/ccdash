<script lang="ts">
	// A past session in the list: where it ran (the machine's colour, the
	// project, the branch) and when it was last active; its title; what you
	// asked first and where it ended (Claude Code's recap, else Claude's last
	// words), or with a search the places it matched; its numbers and marks.
	import GitBranchIcon from '@lucide/svelte/icons/git-branch';
	import type { PastSession } from '#lib/api/types.js';
	import { fmtBytes, fmtDuration, fmtUsd, place, plain, plural } from '#lib/format.js';
	import { fmtFull, fmtWhenShort, headless, marks, modelName, pastTitle } from '#lib/past.js';
	import { hostColor } from '#lib/workspace.js';

	let {
		session: s,
		hosts,
		href,
		selected = false,
		now,
		onselect
	}: {
		session: PastSession;
		/** Every machine of the list, for stable colours. */
		hosts: string[];
		href: string;
		selected?: boolean;
		now: number;
		onselect: (event: MouseEvent) => void;
	} = $props();

	const where = $derived(
		s.workspace ? [s.workspace, s.project].filter(Boolean).join(' › ') : place(s.cwd)
	);
	const others = $derived(s.hosts.filter((h) => h !== s.host));
	const numbers = $derived(
		[
			plural(s.prompts, 'prompt'),
			s.active_ms > 0 ? `${fmtDuration(s.active_ms)} working` : null,
			fmtBytes(s.size),
			s.cost_usd ? fmtUsd(s.cost_usd) : null,
			modelName(s.model)
		].filter(Boolean)
	);
	const ended = $derived(
		s.recap ? ['Recap', s.recap] : s.last_reply ? ['Claude', s.last_reply] : null
	);
	const ROLE: Record<string, string> = { you: 'You', claude: 'Claude', title: 'Title' };
</script>

<a
	{href}
	onclick={onselect}
	class="flex min-w-0 flex-col gap-1.5 rounded-lg border bg-card p-3 text-left text-sm transition-colors hover:bg-accent/40 aria-[current=true]:border-primary/60 aria-[current=true]:bg-accent/50"
	aria-current={selected ? 'true' : undefined}
	data-session={s.id}
>
	<span class="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
		<span
			class="size-2 shrink-0 rounded-full"
			style="background: {hostColor(hosts, s.host)}"
			aria-hidden="true"
		></span>
		<span class="shrink-0 font-medium text-foreground/80" title="Where it last ran">{s.host}</span>
		{#if others.length}
			<span class="shrink-0" title="Also ran on {others.join(', ')}">+{others.length}</span>
		{/if}
		<span aria-hidden="true">·</span>
		<span class="truncate" title={s.cwd}>{where}</span>
		{#if s.branch}
			<span class="inline-flex max-w-[35%] min-w-0 items-center gap-0.5" title="Branch {s.branch}">
				<GitBranchIcon class="size-3 shrink-0" aria-label="branch" />
				<span class="truncate">{s.branch}</span>
			</span>
		{/if}
		<span class="ml-auto shrink-0 tabular-nums" title="Last active {fmtFull(s.last_at)}"
			>{fmtWhenShort(s.last_at, now)}</span
		>
	</span>

	<span class="line-clamp-2 font-medium">{pastTitle(s)}</span>

	{#if s.snippets.length}
		{#each s.snippets as m (m.text + m.role)}
			<span class="line-clamp-2 text-xs text-muted-foreground">
				<span class="font-medium text-foreground/70"
					>{[ROLE[m.role] ?? m.role, m.at ? fmtWhenShort(m.at, now) : null]
						.filter(Boolean)
						.join(' · ')}</span
				>
				{#each marks(m.snippet) as part, i (i)}{#if i % 2}<mark
							class="rounded bg-warning/30 px-0.5 text-foreground">{part}</mark
						>{:else}{part}{/if}{/each}
			</span>
		{/each}
	{:else}
		{#if s.first_prompt}
			<span class="line-clamp-1 text-xs text-muted-foreground">
				<span class="font-medium text-foreground/70">You</span>
				{plain(s.first_prompt)}
			</span>
		{/if}
		{#if ended}
			<span class="line-clamp-2 text-xs text-muted-foreground">
				<span class="font-medium text-foreground/70">{ended[0]}</span>
				{plain(ended[1])}
			</span>
		{/if}
	{/if}

	<span class="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
		<span>{numbers.join(' · ')}</span>
		{#if s.open}
			<span class="rounded-full bg-working/15 px-1.5 text-[11px] font-medium text-working"
				>running</span
			>
		{/if}
		{#if s.continued_in}
			<span
				class="rounded-full bg-muted px-1.5 text-[11px]"
				title="Resumed later as a copy: {s.continued_in}">continued</span
			>
		{/if}
		{#if headless(s)}
			<span
				class="rounded-full bg-muted px-1.5 text-[11px]"
				title="Ran without a terminal ({s.entrypoint})">headless</span
			>
		{/if}
		{#if !s.workspace}
			<span class="rounded-full bg-muted px-1.5 text-[11px]" title={s.cwd}>outside workspaces</span>
		{/if}
	</span>
</a>
