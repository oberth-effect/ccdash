<script lang="ts">
	// A machine's power from the panel: Lock its session, Suspend (asks
	// first, naming the sessions it would pause), Wake while it is down and
	// the hub can reach its card, Forget for one that is gone. `compact` (a
	// card) offers Wake, and Lock while the session is unlocked.
	import { goto } from '$app/navigation';
	import LoaderCircleIcon from '@lucide/svelte/icons/loader-circle';
	import LockIcon from '@lucide/svelte/icons/lock';
	import MoonIcon from '@lucide/svelte/icons/moon';
	import SunIcon from '@lucide/svelte/icons/sun';
	import { Unauthorized } from '#lib/api/client.js';
	import {
		forgetMachine,
		lockMachine,
		suspendMachine,
		wakeMachine,
		type HostStatus
	} from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { fmtDuration } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import {
		busyOn,
		canLock,
		canSuspend,
		canWake,
		isAsleep,
		isWaking,
		wakeFailed,
		wakeWhy
	} from '#lib/machines.js';
	import { label } from '#lib/session.js';

	let { host: h, compact = false }: { host: HostStatus; compact?: boolean } = $props();

	let asking = $state<'suspend' | 'forget' | null>(null);
	let doing = $state<'lock' | 'suspend' | 'wake' | 'forget' | null>(null);
	let error = $state('');
	/** Locked from here; shown until the machine's report says so. */
	let lockedHere = $state(false);
	const busy = $derived(doing !== null);
	const locked = $derived(h.machine?.locked === true || lockedHere);
	/** Unlocked and lockable from here: what a card offers to fix. */
	const lockable = $derived(canLock(h) && h.machine?.locked === false && !lockedHere);

	const working = $derived(busyOn(live.list, h.name));
	const wakeTo = $derived(h.wake?.kind === 'ready' ? h.wake.to.replace(/:9$/, '') : null);

	async function act(what: NonNullable<typeof doing>, run: () => Promise<void>) {
		doing = what;
		error = '';
		try {
			await run();
			asking = null;
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		} finally {
			doing = null;
		}
	}

	const lock = () =>
		act('lock', async () => {
			const { host } = await lockMachine(h.name);
			live.putHost(host);
			lockedHere = true;
		});
	const suspend = () =>
		act('suspend', async () => {
			const { host } = await suspendMachine(h.name, working.length > 0);
			live.putHost(host);
		});
	const wake = () =>
		act('wake', async () => {
			const { host } = await wakeMachine(h.name);
			live.putHost(host);
		});
	const forget = () =>
		act('forget', async () => {
			await forgetMachine(h.name);
			live.hosts = live.hosts.filter((x) => x.name !== h.name);
			await goto('/machines');
		});

	// NOTE: the report after a lock says so within seconds; this only bridges the gap
	$effect(() => {
		if (!lockedHere) return;
		if (h.machine?.locked === true) {
			lockedHere = false;
			return;
		}
		const timer = setTimeout(() => (lockedHere = false), 15_000);
		return () => clearTimeout(timer);
	});

	// NOTE: a card's outcome is a passing note, a page's stays until the next try
	$effect(() => {
		if (!compact || !error) return;
		const timer = setTimeout(() => (error = ''), 8000);
		return () => clearTimeout(timer);
	});
</script>

{#snippet cardButton(text: string, what: 'lock' | 'wake', run: () => void)}
	<Button
		variant="outline"
		size="xs"
		disabled={busy}
		onclick={(e: MouseEvent) => {
			e.preventDefault();
			run();
		}}
	>
		{#if doing === what}<LoaderCircleIcon
				class="size-3.5 animate-spin"
			/>{:else if what === 'lock'}<LockIcon class="size-3.5" />{:else}<SunIcon
				class="size-3.5"
			/>{/if}
		{text}
	</Button>
{/snippet}

{#if compact}
	<!-- NOTE: the card's header already says it is waking -->
	{#if canWake(h, live.now) || lockable || error}
		<span class="relative z-10 flex flex-wrap items-center gap-2 text-xs">
			{#if lockable}
				{@render cardButton('Lock', 'lock', lock)}
			{/if}
			{#if canWake(h, live.now)}
				{@render cardButton(wakeFailed(h, live.now) ? 'Wake again' : 'Wake', 'wake', wake)}
			{/if}
			{#if error}<span class="text-critical" role="alert">{error}</span>{/if}
		</span>
	{/if}
{:else}
	<div class="flex flex-col gap-1.5">
		<div class="flex flex-wrap items-center gap-2">
			{#if canLock(h) && !locked}
				<Button variant="outline" size="sm" disabled={busy} onclick={lock}>
					{#if doing === 'lock'}<LoaderCircleIcon class="size-4 animate-spin" />{:else}<LockIcon
							class="size-4"
						/>{/if}
					Lock
				</Button>
			{/if}
			{#if canSuspend(h)}
				<Button
					variant="outline"
					size="sm"
					disabled={busy}
					onclick={() => {
						error = '';
						asking = 'suspend';
					}}
				>
					<MoonIcon class="size-4" /> Suspend…
				</Button>
			{/if}
			{#if isWaking(h, live.now) && h.power?.state === 'waking'}
				<span class="flex items-center gap-1.5 text-sm text-muted-foreground">
					<LoaderCircleIcon class="size-4 animate-spin" />
					Waking… {fmtDuration(live.now - h.power.since)}
				</span>
			{:else if canWake(h, live.now)}
				<Button variant="outline" size="sm" disabled={busy} onclick={wake}>
					{#if doing === 'wake'}<LoaderCircleIcon class="size-4 animate-spin" />{:else}<SunIcon
							class="size-4"
						/>{/if}
					{wakeFailed(h, live.now) ? 'Wake again' : 'Wake'}
				</Button>
			{/if}
			{#if !h.connected}
				<Button
					variant="ghost"
					size="sm"
					class="ml-auto text-muted-foreground"
					disabled={busy}
					onclick={() => {
						error = '';
						asking = 'forget';
					}}>Forget…</Button
				>
			{/if}
		</div>
		{#if error && !asking}<p class="text-sm text-critical" role="alert">{error}</p>{/if}
		<p class="text-xs text-muted-foreground">
			{#if h.connected && !isAsleep(h) && h.machine?.locked !== null && h.machine?.locked !== undefined}
				{#if locked}Its session is locked.{:else}<span class="text-permission"
						>Its session is unlocked.</span
					>{/if}
			{/if}
			{#if h.connected && !isAsleep(h) && h.machine && !h.machine.can_lock}
				Locking is off here (<code>power.lock</code>).
			{/if}
			{#if h.connected && !isAsleep(h) && h.machine && !h.machine.can_suspend}
				Suspending is off here (<code>power.suspend</code>).
			{/if}
			{#if wakeTo}
				Wake-on-LAN: magic packets for {h.wake?.kind === 'ready' ? h.wake.mac : ''} to {wakeTo}.
			{:else if wakeWhy(h)}
				The hub cannot wake it: {wakeWhy(h)}.
			{/if}
		</p>
	</div>
{/if}

<Dialog.Root
	open={asking !== null}
	onOpenChange={(open) => {
		if (!open) {
			asking = null;
			error = '';
		}
	}}
>
	<Dialog.Content>
		{#if asking === 'suspend'}
			<Dialog.Header>
				<Dialog.Title>Suspend {h.name}?</Dialog.Title>
				<Dialog.Description>
					Its collector hands the hub what is pending of Claude's state, then puts it to sleep.
					{#if wakeTo}Wake brings it back from here.{:else}<strong
							>The hub cannot wake it again{wakeWhy(h) ? `: ${wakeWhy(h)}` : ''}.</strong
						>{/if}
				</Dialog.Description>
			</Dialog.Header>
			{#if working.length}
				<div class="text-sm">
					<p>
						{working.length === 1
							? 'This session is mid-turn and pauses'
							: 'These sessions are mid-turn and pause'} until it wakes:
					</p>
					<ul class="mt-1 list-disc pl-5">
						{#each working as s (s.id)}<li>{label(s)}</li>{/each}
					</ul>
				</div>
			{/if}
			{#if error}<p class="text-sm text-critical" role="alert">{error}</p>{/if}
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (asking = null)}>Cancel</Button>
				<Button disabled={busy} onclick={suspend}>
					{#if busy}<LoaderCircleIcon class="size-4 animate-spin" />{/if}
					{working.length ? 'Suspend anyway' : 'Suspend'}
				</Button>
			</Dialog.Footer>
		{:else if asking === 'forget'}
			<Dialog.Header>
				<Dialog.Title>Forget {h.name}?</Dialog.Title>
				<Dialog.Description>
					The hub drops it from Machines, with its last report and how to wake it. It comes back
					when its collector connects again; its sessions and history stay.
				</Dialog.Description>
			</Dialog.Header>
			{#if error}<p class="text-sm text-critical" role="alert">{error}</p>{/if}
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (asking = null)}>Cancel</Button>
				<Button variant="destructive" disabled={busy} onclick={forget}>Forget</Button>
			</Dialog.Footer>
		{/if}
	</Dialog.Content>
</Dialog.Root>
