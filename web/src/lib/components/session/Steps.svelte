<script lang="ts">
	// A run of tool calls, permission requests, answers and task updates
	// between Claude's messages: one line for the run, then a row per step
	// that reads as a sentence (what Claude said the call does, else what it
	// touched; same-tool file operations folded) and unfolds to the command,
	// its output and time. Folded for older exchanges, open for the live one.
	import CheckIcon from '@lucide/svelte/icons/check';
	import HandIcon from '@lucide/svelte/icons/hand';
	import SquareIcon from '@lucide/svelte/icons/square';
	import SquareCheckIcon from '@lucide/svelte/icons/square-check';
	import type { Item } from '#lib/api/types.js';
	import { fmtDuration, fmtTime, known, plural, toolName } from '#lib/format.js';
	import { ICONS, rows as toRows, summarize } from '#lib/steps.js';
	import CodeBlock from './CodeBlock.svelte';

	const VISIBLE = 20;

	let {
		items,
		omitted = 0,
		open = false
	}: { items: Item[]; omitted?: number; open?: boolean } = $props();

	let showAll = $state(false);

	const all = $derived(toRows(items));
	const hidden = $derived(showAll ? 0 : Math.max(0, all.length - VISIBLE));
	const shown = $derived(all.slice(hidden));
	const summary = $derived(summarize(items, omitted));
	const range = $derived.by(() => {
		const from = fmtTime(items[0].at);
		const to = fmtTime(items[items.length - 1].at);
		return from === to ? from : `${from}–${to}`;
	});

	const answered = (item: Extract<Item, { kind: 'answered' }>) =>
		({
			allow: item.tool === 'ExitPlanMode' ? 'Approved the plan' : 'Allowed',
			always: 'Always allowed',
			deny: item.tool === 'ExitPlanMode' ? 'Kept planning' : 'Denied',
			answer: 'Answered'
		})[item.action] ?? 'Answered';
	const taskVerb = (status: string) =>
		({ pending: 'added', in_progress: 'started', completed: 'completed', deleted: 'removed' })[
			status
		] ?? status;
</script>

<details class="group rounded-md border bg-muted/40 text-xs" {open}>
	<summary
		class="flex cursor-pointer flex-wrap items-baseline gap-x-2 px-2.5 py-1.5 select-none [&::-webkit-details-marker]:hidden"
	>
		<span class="min-w-0 flex-1 text-muted-foreground">{summary}</span>
		<span class="shrink-0 text-muted-foreground tabular-nums">{range}</span>
	</summary>
	<ol class="flex flex-col px-1.5 pb-1.5">
		{#if hidden}
			<li class="px-1 py-1">
				<button
					type="button"
					class="text-muted-foreground underline"
					onclick={() => (showAll = true)}>Show {plural(hidden, 'earlier step')}</button
				>
			</li>
		{/if}
		{#each shown as row (row.key)}
			<li>
				{#if row.kind === 'other'}
					{@const item = row.item}
					<div class="flex items-start gap-2 px-1 py-1">
						{#if item.kind === 'permission'}
							<HandIcon class="mt-0.5 size-3.5 shrink-0 text-permission" />
							<span class="min-w-0 flex-1"
								>Asked permission: {toolName(item.tool)}{#if item.summary}<span
										class="font-mono break-all"
									>
										{item.summary}</span
									>{/if}</span
							>
						{:else if item.kind === 'answered'}
							<CheckIcon class="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
							<span class="min-w-0 flex-1"
								>{answered(item)} from {item.via === 'notification'
									? 'a notification'
									: 'ccdash'}</span
							>
						{:else if item.kind === 'task'}
							{#if item.status === 'completed'}
								<SquareCheckIcon class="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
							{:else}
								<SquareIcon class="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
							{/if}
							<span class="min-w-0 flex-1">Task {taskVerb(item.status)}: {item.subject}</span>
						{/if}
						<time class="shrink-0 text-muted-foreground tabular-nums">{fmtTime(item.at)}</time>
					</div>
				{:else}
					{@const Icon = ICONS[row.family]}
					{@const told = row.told}
					{@const failed = Boolean(told.error)}
					<details class="rounded">
						<summary
							class="flex cursor-pointer items-start gap-2 rounded px-1 py-1 select-none hover:bg-accent/40 [&::-webkit-details-marker]:hidden"
						>
							<Icon
								class="mt-0.5 size-3.5 shrink-0 {failed
									? 'text-critical'
									: 'text-muted-foreground'}"
							/>
							<span class="min-w-0 flex-1 {failed ? 'text-critical' : ''}">
								{#if told.text}<span>{told.text}</span>{/if}
								{#if told.code}<span class="font-mono break-all {told.text ? '' : 'line-clamp-2'}"
										>{told.text ? ' ' : ''}{told.code}</span
									>{/if}
								{#if told.detail}<span class="block truncate font-mono text-muted-foreground"
										>{told.detail}</span
									>{/if}
								{#if told.error}<span class="block truncate">✗ {told.error}</span>{/if}
							</span>
							{#if known(row.ms) && row.ms >= 1000}
								<span class="shrink-0 text-muted-foreground tabular-nums"
									>{fmtDuration(row.ms)}</span
								>
							{/if}
						</summary>
						<div
							class="mb-1 ml-5.5 flex min-w-0 flex-col gap-1.5 rounded border bg-background/70 p-2 text-muted-foreground"
						>
							{#if row.kind === 'fold'}
								<ul class="flex flex-col gap-0.5">
									{#each row.items as item (item.id)}
										<li class="flex items-baseline gap-2">
											<time class="shrink-0 tabular-nums">{fmtTime(item.at)}</time>
											<span class="min-w-0 font-mono break-all text-foreground">{item.summary}</span
											>
											{#if known(item.ms) && item.ms >= 1000}<span
													class="ml-auto shrink-0 tabular-nums">{fmtDuration(item.ms)}</span
												>{/if}
										</li>
									{/each}
								</ul>
							{:else}
								{@const item = row.item}
								<span class="tabular-nums"
									>{fmtTime(item.at)} · {toolName(item.name)}{known(item.ms) && item.ms >= 1000
										? ` · ${fmtDuration(item.ms)}`
										: ''}{item.ok ? '' : ' · failed'}</span
								>
								{#if item.command || item.summary}
									<CodeBlock
										code={item.command ?? item.summary}
										lang={item.command ? 'bash' : null}
										class="my-0 text-foreground"
									/>
								{/if}
								{#if item.output}
									<CodeBlock
										code={item.output}
										bodyClass="max-h-60"
										class="my-0 {item.ok ? 'text-foreground' : 'text-critical'}"
									/>
								{/if}
							{/if}
						</div>
					</details>
				{/if}
			</li>
		{/each}
	</ol>
</details>
