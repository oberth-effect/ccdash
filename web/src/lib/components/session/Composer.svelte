<script lang="ts">
	// Text for the session: Send starts a turn through the channel, or wakes
	// a sleeping session with it; Next prompt leaves it as context for the
	// next prompt (also for sessions without the channel); Stop interrupts a
	// turn, so the correction typed next is what Claude reads. Quick replies
	// send at once; the field (history, templates, note links, fence,
	// preview, vim keys, its shortcuts) is `PromptField`. The draft lives on
	// the hub (`drafts.ts`): it survives reloads and follows the owner from
	// the phone to the desk, and Edit in nvim opens it in an editor on a
	// machine. `i` anywhere on the page goes back to the field, as Esc left it.
	import OctagonXIcon from '@lucide/svelte/icons/octagon-x';
	import { Unauthorized } from '#lib/api/client.js';
	import {
		cancelEdit,
		editDraft,
		fetchPrompts,
		interruptSession,
		messageTo,
		replyTo,
		type Session
	} from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import {
		clearDraft,
		editHostFor,
		flushDraft,
		initialDraft,
		markSynced,
		noteDraft,
		remoteDraft,
		sessionDraft
	} from '#lib/drafts.js';
	import { live } from '#lib/live/store.svelte.js';
	import { prefs } from '#lib/prefs.svelte.js';
	import { busy as working } from '#lib/session.js';
	import PromptField from './PromptField.svelte';

	let { session: s, onsent }: { session: Session; onsent?: (kind: 'reply' | 'message') => void } =
		$props();

	const key = $derived(sessionDraft(s.id));
	// svelte-ignore state_referenced_locally
	let text = $state(initialDraft(sessionDraft(s.id)));
	let field = $state<PromptField | null>(null);
	let busy = $state(false);
	let error = $state('');

	const canReply = $derived((s.channel || s.asleep) && s.activity !== 'gone');
	const canStop = $derived(working(s) && s.hooked && live.connected(s.host));
	const draft = $derived(live.drafts.get(key));
	const editHost = $derived(editHostFor(s.host));
	let stopping = $state(false);

	$effect(() => {
		noteDraft(key, text);
	});

	// what the hub says (another device, an editor) lands unless typed over
	$effect(() => {
		const adopted = remoteDraft(key, draft);
		if (adopted !== null) text = adopted;
	});

	async function stop() {
		if (stopping || s.interrupting) return;
		stopping = true;
		error = '';
		try {
			live.put((await interruptSession(s.id)).session);
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		} finally {
			stopping = false;
		}
	}

	function quickReply(index: number) {
		const q = prefs.quick[index];
		if (!q || busy) return;
		text = q;
		submit(canReply ? 'reply' : 'message');
	}

	function onwindowkey(event: KeyboardEvent) {
		if (event.key !== 'i' || event.ctrlKey || event.metaKey || event.altKey) return;
		const target = event.target as HTMLElement;
		if (target.closest('input, textarea, select, [contenteditable], [role=dialog]')) return;
		event.preventDefault();
		field?.focus();
	}

	async function prompts() {
		const own = (await fetchPrompts({ session: s.id, limit: 50 })).prompts;
		const ws = s.workspace
			? (await fetchPrompts({ workspace: s.workspace, limit: 50 })).prompts
			: [];
		return [...own, ...ws];
	}

	async function submit(kind: 'reply' | 'message') {
		const body = text.trim();
		if (!body || busy || draft?.editing) return;
		// NOTE: the field goes read-only while busy and loses focus; it gets it back
		// where it had it (not on a phone that tapped the button)
		const refocus = field?.hasFocus() ?? false;
		busy = true;
		error = '';
		try {
			if (kind === 'reply') live.put((await replyTo(s.id, body)).session);
			else await messageTo(s.id, body);
			text = '';
			clearDraft(key);
			field?.remember(body);
			onsent?.(kind);
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		} finally {
			busy = false;
			if (refocus) requestAnimationFrame(() => field?.focus());
		}
	}

	async function edit() {
		if (!editHost || busy) return;
		error = '';
		try {
			const sent = text;
			const { draft } = await editDraft(key, editHost, sent);
			markSynced(key, sent);
			live.putDraft(draft);
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		}
	}

	async function cancel() {
		error = '';
		try {
			live.putDraft((await cancelEdit(key)).draft);
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		}
	}
</script>

<svelte:window onkeydown={onwindowkey} />

<div class="flex flex-col gap-1.5">
	{#if prefs.quick.length}
		<div class="flex gap-1.5 overflow-x-auto pb-0.5">
			{#each prefs.quick as q, i (q)}
				<Button
					variant="outline"
					size="xs"
					class="shrink-0"
					disabled={busy || Boolean(draft?.editing)}
					title={i < 9 ? `Ctrl Shift ${i + 1}` : undefined}
					onclick={() => quickReply(i)}>{q}</Button
				>
			{/each}
		</div>
	{/if}
	<PromptField
		bind:this={field}
		bind:text
		{prompts}
		historyNote="This session's prompts, then the workspace's."
		session={s.id}
		host={s.host}
		workspace={s.workspace}
		placeholder={s.asleep
			? 'Message Claude… (wakes the session)'
			: canReply
				? 'Message Claude…'
				: 'Note for the next prompt…'}
		label="Message to Claude"
		disabled={busy}
		editing={draft?.editing ?? null}
		{editHost}
		notice={draft?.notice ?? null}
		onsubmit={() => submit(canReply ? 'reply' : 'message')}
		onalternate={canReply ? () => submit('message') : undefined}
		onquick={quickReply}
		onstop={canStop ? stop : undefined}
		onedit={edit}
		oncanceledit={cancel}
		onsave={() => flushDraft(key)}
	>
		{#snippet actions()}
			{#if canStop}
				<Button
					type="button"
					size="sm"
					variant="outline"
					class="text-critical"
					disabled={stopping || Boolean(s.interrupting)}
					title={s.interrupting
						? 'Asked already: Claude stops at its next step'
						: 'Interrupt the turn: Claude stops at its next step, then reads what you send'}
					onclick={stop}><OctagonXIcon /> Stop</Button
				>
			{/if}
			{#if canReply}
				<Button
					type="submit"
					size="sm"
					disabled={busy || !text.trim() || Boolean(draft?.editing)}
					title="Send (Ctrl Enter)">Send</Button
				>
			{/if}
			<Button
				type="button"
				size="sm"
				variant={canReply ? 'outline' : 'default'}
				disabled={busy || !text.trim() || Boolean(draft?.editing)}
				title="Hand the text to Claude as context with its next prompt, without starting a turn (Ctrl Shift Enter)"
				onclick={() => submit('message')}>Next prompt</Button
			>
		{/snippet}
	</PromptField>
	{#if error}<p class="text-xs text-critical" role="alert">{error}</p>{/if}
	{#if !s.channel && !s.asleep && s.activity !== 'gone'}
		<p class="text-xs text-muted-foreground">
			To start a turn from here, the session needs the ccdash channel: exit it and run
			<code class="font-mono">claude --resume {s.id}</code>. A note still reaches it with its next
			prompt.
		</p>
	{/if}
</div>
