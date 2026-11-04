<script lang="ts">
	// Reports of a project: hot first (published or recalled lately), then
	// the rest, newest first; each opens in the viewer.
	import type { Doc } from '#lib/api/types.js';
	import ReportRow from '#lib/components/reports/ReportRow.svelte';

	let { reports, showProject }: { reports: Doc[]; showProject: boolean } = $props();

	const hot = $derived(reports.filter((d) => d.hot));
	const rest = $derived(reports.filter((d) => !d.hot));
</script>

{#snippet list(title: string, items: Doc[])}
	{#if items.length}
		<section>
			<h3 class="mb-1 text-sm font-semibold">
				{title} <span class="font-normal text-muted-foreground">{items.length}</span>
			</h3>
			<ul class="divide-y rounded-lg border bg-card">
				{#each items as d (d.slug)}
					<li><ReportRow card={d} project={showProject} /></li>
				{/each}
			</ul>
		</section>
	{/if}
{/snippet}

{#if !reports.length}
	<p class="text-sm text-muted-foreground">
		No reports yet. A session publishes one with the <code>ccdash:report</code> skill (<code
			>report_publish</code
		>); it then shows here and under Reports.
	</p>
{/if}
<div class="flex flex-col gap-4">
	{@render list('Hot', hot)}
	{@render list(hot.length ? 'Others' : 'Reports', rest)}
</div>
