<script lang="ts">
	// The last progress across the scope as a short timeline: days, each
	// entry with its time, project, title and first lines, the issue it is
	// on and the session that logged it. A title opens the note beside the
	// page; everything else is on the project's Progress tab.
	import BotIcon from '@lucide/svelte/icons/bot';
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import CircleDotIcon from '@lucide/svelte/icons/circle-dot';
	import type { Entry } from '#lib/api/types.js';
	import { fmtTime, plural } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { notePanel } from '#lib/notepanel.svelte.js';
	import { dayKey, dayName, entryRest } from '#lib/notes.js';

	let { entries }: { entries: Entry[] } = $props();

	const days = $derived.by(() => {
		const out: { key: string; day: string; items: Entry[] }[] = [];
		for (const e of entries) {
			const key = dayKey(e.logged_at);
			const last = out.at(-1);
			if (last?.key === key) last.items.push(e);
			else out.push({ key, day: dayName(e.logged_at, live.now), items: [e] });
		}
		return out;
	});
</script>

<div class="flex flex-col gap-3">
	{#each days as group (group.key)}
		<section>
			<h3
				class="mb-1.5 flex items-baseline gap-2 text-xs font-semibold text-muted-foreground uppercase"
			>
				{group.day}
				<span class="font-normal normal-case">{plural(group.items.length, 'entry', 'entries')}</span
				>
			</h3>
			<ol class="ml-1.5 border-l">
				{#each group.items as e (e.workspace + e.slug)}
					{@const closed = e.on?.status === 'archived'}
					{@const rest = entryRest(e)}
					<li class="relative flex min-w-0 flex-col gap-0.5 pb-3 pl-4 text-sm last:pb-0">
						<span
							class="absolute top-1.5 -left-[4.5px] size-2 rounded-full {closed
								? 'bg-your-turn'
								: 'bg-kind-progress'}"
							aria-hidden="true"
						></span>
						<span class="flex min-w-0 items-baseline gap-1.5 text-xs text-muted-foreground">
							<span class="font-mono tabular-nums">{fmtTime(e.logged_at)}</span>
							<span class="truncate">{e.project ?? e.workspace}</span>
						</span>
						<button
							type="button"
							class="text-left font-medium break-words hover:underline"
							onclick={() => notePanel.show(e.workspace, e.slug)}>{e.title}</button
						>
						{#if rest}<p class="line-clamp-2 text-xs text-muted-foreground">{rest}</p>{/if}
						{#if e.on || e.by}
							<span class="flex min-w-0 flex-wrap items-center gap-x-3 text-xs">
								{#if e.on}
									{@const on = e.on}
									<button
										type="button"
										class="inline-flex max-w-full min-w-0 items-center gap-1 hover:underline"
										title={closed ? 'Closed issue' : 'Open issue'}
										onclick={() => notePanel.show(e.workspace, on.slug)}
									>
										{#if closed}<CircleCheckIcon class="size-3 shrink-0 text-your-turn" />
										{:else}<CircleDotIcon class="size-3 shrink-0 text-kind-issue" />{/if}
										<span class="truncate">{on.title ?? on.slug}</span>
									</button>
								{/if}
								{#if e.by}
									{@const by = e.by}
									<a
										href="/sessions/{by.session}"
										class="inline-flex min-w-0 items-center gap-1 text-muted-foreground hover:underline"
									>
										<BotIcon class="size-3 shrink-0" />
										<span class="truncate">{by.label}</span>
										{#if by.live}<i
												class="inline-block size-1.5 rounded-full bg-working"
												aria-label="still running"
											></i>{/if}
									</a>
								{/if}
							</span>
						{/if}
					</li>
				{/each}
			</ol>
		</section>
	{/each}
</div>
