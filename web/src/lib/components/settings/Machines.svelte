<script lang="ts">
	// Settings › Machines: the hosts the hub knows and their link, which
	// one this device sits on, and where the Claude Code state lives.
	import { fmtDuration } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
</script>

<section class="flex flex-col gap-2">
	<h2 class="text-base font-semibold">Hosts</h2>
	<ul class="text-sm">
		{#each live.hosts as h (h.name)}
			<li>
				<i
					class="mr-1.5 inline-block size-2 rounded-full {h.connected
						? 'bg-your-turn'
						: 'bg-critical'}"
					aria-hidden="true"
				></i>{h.name}
				<span class="text-muted-foreground">
					· {h.connected ? 'connected' : 'offline'} for {fmtDuration(
						live.now - h.since
					)}{#if h.edit}
						· opens an editor{/if}{#if live.here === h.name}
						· this device is here{/if}</span
				>
			</li>
		{:else}
			<li class="text-muted-foreground">No collector has reported yet.</li>
		{/each}
	</ul>
	<p class="text-xs text-muted-foreground">
		Hub: {live.host}. Uptime, load, power and wake are on the
		<a href="/machines" class="underline underline-offset-2">Machines</a> page.
	</p>
</section>

<section class="flex flex-col gap-2">
	<h2 class="text-base font-semibold">Claude Code state</h2>
	<p class="text-sm text-muted-foreground">
		Each machine's copy, Resync, conflicts and sizes are on the
		<a href="/state" class="underline underline-offset-2">State</a> page.
	</p>
</section>
