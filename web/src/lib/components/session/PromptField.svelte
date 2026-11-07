<script module lang="ts">
	import type { Skill } from '#lib/api/index.js';

	/** Skills per host and workspace, shared by every field, kept five minutes. */
	const skillCache: Record<string, { at: number; skills: Skill[] }> = {};
	const SKILLS_TTL = 5 * 60_000;
	/** How many skills the `/` menu shows at once. */
	const SLASH_ROWS = 8;
	/** The hub refuses longer prompts; the count shows from `WARN_AT`. */
	const MAX_CHARS = 20_000;
	const WARN_AT = 15_000;
</script>

<script lang="ts">
	// A prompt: the editor (`Editor`, CodeMirror with the owner's vim keys
	// when on) and its toolbar, shared by the session composer and the
	// start sheet. Enter is always a new line; Ctrl+Enter submits,
	// Ctrl+Shift+Enter takes the other way (Next prompt). Ctrl+↑/↓ walk
	// the prompt history (Ctrl+R searches it); templates, `[[` for a note
	// link, a code fence around the selection, a markdown preview. Ctrl+G
	// opens the text in an editor on a machine (`onedit`), Ctrl+Shift+F
	// expands the field to most of the screen. A message that starts with
	// `/` offers the skills of the field's host and workspace, as the
	// terminal does; arrows, Enter, Tab or a tap complete one. The keyboard
	// button lists the rest (`KEYS`).
	import type { Snippet } from 'svelte';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import CodeIcon from '@lucide/svelte/icons/code';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import HistoryIcon from '@lucide/svelte/icons/history';
	import KeyboardIcon from '@lucide/svelte/icons/keyboard';
	import LinkIcon from '@lucide/svelte/icons/link';
	import Maximize2Icon from '@lucide/svelte/icons/maximize-2';
	import Minimize2Icon from '@lucide/svelte/icons/minimize-2';
	import SquareTerminalIcon from '@lucide/svelte/icons/square-terminal';
	import { marked } from 'marked';
	import { Unauthorized } from '#lib/api/client.js';
	import { fetchSkills, type Editing, type Prompt } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import * as Sheet from '#lib/components/ui/sheet/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { fmtDuration, fmtStamp } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { palette } from '#lib/palette.svelte.js';
	import { prefs } from '#lib/prefs.svelte.js';
	import { modeLabel } from '#lib/vim.js';
	import Editor from './Editor.svelte';

	let {
		text = $bindable(''),
		prompts,
		historyNote,
		session,
		host,
		workspace = null,
		placeholder,
		label = 'Prompt',
		disabled = false,
		rows = 1,
		fill = false,
		expandable = true,
		editing = null,
		editHost = null,
		notice = null,
		onsubmit,
		onalternate,
		onquick,
		onstop,
		onedit,
		oncanceledit,
		onsave,
		actions
	}: {
		text?: string;
		/** The history, loaded on first use (duplicates are dropped). */
		prompts: () => Promise<Prompt[]>;
		historyNote: string;
		/** The session the field writes to: other sessions' prompts are marked. */
		session?: string;
		/** Whose skills `/` offers: the machine the session runs on, its workspace. */
		host?: string;
		workspace?: string | null;
		placeholder: string;
		label?: string;
		disabled?: boolean;
		/** Lines the empty field shows; it grows with its text up to 220 px. */
		rows?: number;
		/** Fill the parent's height (the start sheet); no Expand then. */
		fill?: boolean;
		expandable?: boolean;
		/** An editor on a machine has the text; the field waits. */
		editing?: Editing | null;
		/** The machine `onedit` opens the editor on; null when none can. */
		editHost?: string | null;
		/** How the last edit ended without text. */
		notice?: string | null;
		onsubmit: () => void;
		/** Ctrl Shift Enter: the other way to send, if there is one. */
		onalternate?: () => void;
		/** Ctrl Shift and a digit: quick reply number `index`, from 0. */
		onquick?: (index: number) => void;
		/** Ctrl . : stop what the session is doing. */
		onstop?: () => void;
		/** Ctrl G: open the text in an editor on `editHost`. */
		onedit?: () => void;
		oncanceledit?: () => void;
		/** `:w` in vim: keep the draft now. */
		onsave?: () => void;
		/** Buttons at the end of the toolbar. */
		actions: Snippet;
	} = $props();

	let editor = $state<Editor | null>(null);
	let inlineHost = $state<HTMLDivElement | null>(null);
	let bigHost = $state<HTMLDivElement | null>(null);
	let expanded = $state(false);
	let preview = $state(false);
	let focused = $state(false);
	let mode = $state('');
	let vimrcErrors = $state<string[]>([]);
	let historyOpen = $state(false);
	let historyQuery = $state('');
	let history = $state<Prompt[]>([]);
	let historyLoaded = false;
	/** Where up/down is in `history`; -1 is the field's own text. */
	let cursor = $state(-1);
	let stash = '';

	let skills = $state<Skill[] | null>(null);
	/** Escape closed the menu; typing opens it again. */
	let dismissed = $state(false);
	let pick = $state(0);

	const touch = () => matchMedia('(pointer: coarse)').matches;
	const vimOn = $derived(prefs.vimOn);
	const locked = $derived(disabled || Boolean(editing));
	const chars = $derived(text.length);
	/** The shortcuts this field has, for the keyboard button. */
	const KEYS = $derived(
		[
			['Ctrl Enter', 'Send'],
			['Enter', 'New line'],
			onalternate ? ['Ctrl Shift Enter', 'Next prompt'] : null,
			onquick ? ['Ctrl Shift 1–9', 'Quick reply'] : null,
			onstop ? ['Ctrl .', 'Stop the turn'] : null,
			onedit ? ['Ctrl G', `Edit in nvim${editHost ? ` on ${editHost}` : ''}`] : null,
			expandable && !fill ? ['Ctrl Shift F', 'Expand the field'] : null,
			['Ctrl J', 'Go to the open question'],
			['Ctrl ↑ ↓', 'Prompt history'],
			['Ctrl R', 'Search the prompt history'],
			['Ctrl E', 'Code fence around the selection'],
			['Ctrl P', 'Preview'],
			['[[', 'Link a note'],
			['/', 'Skills, at the start'],
			vimOn
				? ['Esc, :w, :x', 'Vim: normal mode, keep the draft, done']
				: ['Esc', 'Leave the field'],
			onquick ? ['i', 'Back into the field'] : null,
			['Ctrl K', 'Search everything']
		].filter((row): row is string[] => row !== null)
	);
	/** What follows a leading slash while nothing else was typed; else null. */
	const slash = $derived(/^\/(\S*)$/.exec(text)?.[1] ?? null);
	const choices = $derived.by(() => {
		if (slash === null || dismissed || !skills) return [];
		const q = slash.toLowerCase();
		const starts = skills.filter((s) => s.command.toLowerCase().startsWith(q));
		const within = skills.filter(
			(s) => !s.command.toLowerCase().startsWith(q) && s.command.toLowerCase().includes(q)
		);
		return [...starts, ...within].slice(0, SLASH_ROWS);
	});
	$effect(() => {
		if (slash !== null && host) loadSkills();
	});
	$effect(() => {
		void choices.length;
		pick = 0;
	});
	const filtered = $derived(
		history.filter(
			(p) => !historyQuery.trim() || p.text.toLowerCase().includes(historyQuery.toLowerCase())
		)
	);
	const rendered = $derived(preview ? (marked.parse(text, { async: false }) as string) : '');

	// the one editor lives in the field or in the expanded dialog
	$effect(() => {
		const el = editor?.element();
		const target = expanded ? bigHost : inlineHost;
		if (!el || !target || el.parentElement === target) return;
		target.appendChild(el);
		editor?.measure();
		requestAnimationFrame(() => editor?.focus());
	});

	/** After a send: the text heads the history. */
	export function remember(body: string) {
		cursor = -1;
		history = history.filter((p) => p.text !== body);
		history.unshift({ at: live.now, session: session ?? '', text: body });
		preview = false;
		expanded = false;
	}

	export function focus() {
		editor?.focus();
	}

	export function hasFocus(): boolean {
		return editor?.hasFocus() ?? false;
	}

	async function loadHistory() {
		if (historyLoaded) return;
		historyLoaded = true;
		try {
			const seen: Record<string, true> = {};
			for (const p of history) seen[p.text] = true;
			for (const p of await prompts()) {
				if (seen[p.text]) continue;
				seen[p.text] = true;
				history.push(p);
			}
		} catch (e) {
			historyLoaded = false;
			if (e instanceof Unauthorized) live.needToken = true;
		}
	}

	async function loadSkills() {
		if (!host) return;
		const key = `${host}\n${workspace ?? ''}`;
		const hit = skillCache[key];
		if (hit && Date.now() - hit.at < SKILLS_TTL) {
			skills = hit.skills;
			return;
		}
		try {
			const { skills: list } = await fetchSkills(host, workspace ?? null);
			skillCache[key] = { at: Date.now(), skills: list };
			skills = list;
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
		}
	}

	function useSkill(s: Skill) {
		dismissed = true;
		editor?.replaceRange(0, text.length, `/${s.command} `);
	}

	function insert(snippet: string) {
		editor?.insert(snippet);
	}

	function fence() {
		if (!editor) return;
		const { from, to } = editor.selection();
		const selected = text.slice(from, to);
		editor.replaceRange(from, to, `\n\`\`\`\n${selected}\n\`\`\`\n`, [5, 5 + selected.length]);
	}

	function noteLink() {
		palette.show({
			notes: true,
			onNote: (hit) => insert(`[[${hit.slug}]] `)
		});
	}

	function walk(direction: 1 | -1) {
		if (!history.length) return;
		if (cursor === -1) stash = text;
		const next = Math.min(history.length - 1, Math.max(-1, cursor + direction));
		cursor = next;
		text = next === -1 ? stash : history[next].text;
	}

	function toggleExpand() {
		if (!expandable || fill) return;
		expanded = !expanded;
		preview = false;
	}

	/** Shortcuts with Ctrl or ⌘. NOTE: digits by position: a Czech keyboard
	 * types `+ě` there. Not Alt: on Linux Firefox it switches tabs. */
	function shortcut(event: KeyboardEvent): boolean {
		const key = event.key.toLowerCase();
		if (event.key === 'Enter') {
			if (event.shiftKey) {
				if (!onalternate) return false;
				onalternate();
			} else onsubmit();
			return true;
		}
		if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
			if (cursor === -1 && event.key === 'ArrowDown') return false;
			loadHistory().then(() => walk(event.key === 'ArrowUp' ? 1 : -1));
			return true;
		}
		if (event.shiftKey) {
			if (event.code === 'KeyF') {
				toggleExpand();
				return true;
			}
			const digit = /^Digit([1-9])$/.exec(event.code);
			if (!digit || !onquick) return false;
			onquick(Number(digit[1]) - 1);
			return true;
		}
		if (key === 'e') fence();
		else if (key === 'p') preview = !preview;
		else if (key === 'g') {
			if (!onedit || !editHost || locked) return false;
			onedit();
		} else if (key === 'r') {
			historyOpen = true;
			loadHistory();
		} else if (key === 'j') {
			const answer = document.querySelector<HTMLElement>('[data-ask] button');
			if (!answer) return false;
			answer.focus();
		} else if (event.code === 'Period' && onstop) onstop();
		else return false;
		return true;
	}

	/** The editor's keys, before vim and the default bindings. */
	function onkey(event: KeyboardEvent): boolean {
		if (event.isComposing) return false;
		if ((event.ctrlKey || event.metaKey) && !event.altKey && shortcut(event)) {
			event.preventDefault();
			return true;
		}
		if (choices.length) {
			if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
				event.preventDefault();
				const step = event.key === 'ArrowDown' ? 1 : choices.length - 1;
				pick = (pick + step) % choices.length;
				return true;
			}
			if (event.key === 'Enter' || event.key === 'Tab') {
				event.preventDefault();
				useSkill(choices[pick]);
				return true;
			}
			if (event.key === 'Escape') {
				// NOTE: a dialog around the field closes on a document-level Escape
				event.preventDefault();
				event.stopPropagation();
				dismissed = true;
				return true;
			}
		}
		if (event.key === 'Escape') {
			if (cursor !== -1) {
				event.stopPropagation();
				cursor = -1;
				text = stash;
				return true;
			}
			if (vimOn) return false;
			(document.activeElement as HTMLElement | null)?.blur();
			return true;
		}
		if (event.key === '[' && editor) {
			const { from, to } = editor.selection();
			if (from === to && text[from - 1] === '[' && text[from - 2] !== '[') {
				event.preventDefault();
				editor.replaceRange(from - 1, from, '');
				noteLink();
				return true;
			}
		}
		return false;
	}

	/** NOTE: the editor is hidden during a preview, so its own keys stop: Ctrl P
	 * would reach the browser and print. */
	function onwindowkey(event: KeyboardEvent) {
		if (!preview || event.defaultPrevented) return;
		const again = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'p';
		if (!again && event.key !== 'Escape') return;
		event.preventDefault();
		preview = false;
		requestAnimationFrame(() => editor?.focus());
	}

	function pickHistory(p: Prompt) {
		text = p.text;
		cursor = -1;
		historyOpen = false;
		requestAnimationFrame(() => editor?.focus());
	}

	/** `:x` and `:q`: done here, back to the page (or out of Expand). */
	function quit() {
		if (expanded) expanded = false;
		else (document.activeElement as HTMLElement | null)?.blur();
	}

	const frameClass =
		'rounded-md border bg-background transition-colors focus-within:ring-2 focus-within:ring-ring/50';
</script>

<svelte:window onkeydown={onwindowkey} />

{#snippet toolbar(big: boolean)}
	<div class="flex flex-wrap items-center gap-1">
		{#if editing}
			<span class="flex items-center gap-2 px-1 text-xs text-muted-foreground" role="status">
				<SquareTerminalIcon class="size-3.5 text-working" />
				Editing in nvim on {editing.host} · {fmtDuration(live.now - editing.since)} · the text lands here
				when it closes
				{#if oncanceledit}
					<Button variant="outline" size="xs" type="button" onclick={oncanceledit}>Cancel</Button>
				{/if}
			</span>
		{:else}
			<Button
				variant="ghost"
				size="icon-sm"
				type="button"
				title="Insert a note link ([[)"
				aria-label="Insert a note link"
				disabled={locked}
				onclick={noteLink}
			>
				<LinkIcon />
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				type="button"
				title="Code fence around the selection (Ctrl E)"
				aria-label="Code fence"
				disabled={locked}
				onclick={fence}
			>
				<CodeIcon />
			</Button>
			<Button
				variant={preview ? 'secondary' : 'ghost'}
				size="icon-sm"
				type="button"
				title="Preview (Ctrl P)"
				aria-label="Preview"
				aria-pressed={preview}
				onclick={() => (preview = !preview)}
			>
				<EyeIcon />
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				type="button"
				title="Prompt history (Ctrl R)"
				aria-label="Prompt history"
				onclick={() => {
					historyOpen = true;
					loadHistory();
				}}
			>
				<HistoryIcon />
			</Button>
			{#if onedit}
				<Button
					variant="ghost"
					size="icon-sm"
					type="button"
					title={editHost
						? `Edit in nvim on ${editHost} (Ctrl G)`
						: 'No connected machine opens an editor: set [edit] command in its collector'}
					aria-label="Edit in nvim"
					disabled={locked || !editHost}
					onclick={onedit}
				>
					<SquareTerminalIcon />
				</Button>
			{/if}
			{#if expandable && !fill}
				<Button
					variant="ghost"
					size="icon-sm"
					type="button"
					title={big ? 'Back to the page (Ctrl Shift F)' : 'Expand the field (Ctrl Shift F)'}
					aria-label={big ? 'Shrink the field' : 'Expand the field'}
					onclick={toggleExpand}
				>
					{#if big}<Minimize2Icon />{:else}<Maximize2Icon />{/if}
				</Button>
			{/if}
			<Popover.Root>
				<Popover.Trigger class="hidden sm:block">
					{#snippet child({ props })}
						<Button
							variant="ghost"
							size="icon-sm"
							type="button"
							title="Keyboard shortcuts"
							aria-label="Keyboard shortcuts"
							{...props}
						>
							<KeyboardIcon />
						</Button>
					{/snippet}
				</Popover.Trigger>
				<Popover.Content align="start" class="w-80 p-2">
					<dl class="grid grid-cols-[auto_1fr] items-baseline gap-x-3 gap-y-1 text-xs">
						{#each KEYS as [keys, what] (keys)}
							<dt class="font-mono whitespace-nowrap text-muted-foreground">{keys}</dt>
							<dd>{what}</dd>
						{/each}
					</dl>
					{#if vimrcErrors.length}
						<p class="mt-2 text-xs text-warning">
							Vimrc lines not taken: {vimrcErrors.join('; ')}
						</p>
					{/if}
				</Popover.Content>
			</Popover.Root>
			{#if prefs.templates.length}
				<DropdownMenu.Root>
					<DropdownMenu.Trigger>
						{#snippet child({ props })}
							<Button variant="ghost" size="sm" type="button" disabled={locked} {...props}>
								Templates <ChevronDownIcon class="size-3" />
							</Button>
						{/snippet}
					</DropdownMenu.Trigger>
					<DropdownMenu.Content align="start">
						{#each prefs.templates as t (t.name)}
							<DropdownMenu.Item onSelect={() => insert(t.text)}>{t.name}</DropdownMenu.Item>
						{/each}
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			{/if}
			{#if vimOn && mode}
				<span
					class="hidden rounded px-1.5 py-0.5 font-mono text-[0.65rem] tracking-wide text-muted-foreground sm:inline {mode ===
					'insert'
						? 'bg-working/15 text-working'
						: 'bg-muted'}"
					aria-live="polite">{modeLabel(mode)}</span
				>
			{/if}
			{#if chars >= WARN_AT}
				<span
					class="font-mono text-xs {chars > MAX_CHARS ? 'text-critical' : 'text-muted-foreground'}"
					>{chars.toLocaleString('en')} / {MAX_CHARS.toLocaleString('en')}</span
				>
			{/if}
		{/if}
		<span class="ml-auto flex gap-1.5">{@render actions()}</span>
	</div>
	{#if notice && !editing}
		<p class="text-xs text-muted-foreground" role="status">Editor: {notice}</p>
	{/if}
{/snippet}

<form
	class="flex min-h-0 flex-col gap-1.5 {fill ? 'flex-1' : ''}"
	onsubmit={(e) => {
		e.preventDefault();
		onsubmit();
	}}
>
	{#if preview && !expanded}
		<div
			class="max-h-56 overflow-auto rounded-md border bg-card p-3 text-sm {fill ? 'flex-1' : ''}"
		>
			<div class="prose-chat">
				<!-- eslint-disable-next-line svelte/no-at-html-tags -->
				{@html rendered}
			</div>
		</div>
	{/if}
	<!-- NOTE: the editor's element moves into the expanded dialog and back; it is created once, here -->
	<div
		bind:this={inlineHost}
		class="{frameClass} {fill ? 'flex min-h-0 flex-1 flex-col' : ''} {preview || expanded
			? 'hidden'
			: ''} {locked ? 'opacity-60' : ''}"
	>
		<Editor
			bind:this={editor}
			bind:value={text}
			{placeholder}
			{label}
			disabled={locked}
			vim={vimOn}
			vimrc={prefs.vimrc}
			numbers={prefs.numbers}
			hop={prefs.editor.hop}
			hopKeys={prefs.editor.hopKeys}
			clipboard={prefs.editor.clipboard}
			pasteReads={prefs.editor.paste === 'clipboard'}
			highlight={prefs.editor.markdown}
			wrap={prefs.editor.wrap}
			fill={fill || expanded}
			{rows}
			{onkey}
			onfocus={(f) => (focused = f)}
			onmode={(m, sub) => (mode = m ? modeLabel(m, sub).toLowerCase() : '')}
			onwrite={onsave}
			onquit={quit}
			onvimrc={(errors) => (vimrcErrors = errors)}
		/>
	</div>
	{#if choices.length && !expanded}
		<ul
			role="listbox"
			aria-label="Skills"
			class="max-h-64 overflow-auto rounded-md border bg-popover text-sm shadow-md"
		>
			{#each choices as s, i (s.command + (s.workspace ?? ''))}
				<li role="option" aria-selected={i === pick}>
					<button
						type="button"
						tabindex="-1"
						class="flex w-full flex-col items-start gap-0.5 px-3 py-1.5 text-left hover:bg-accent {i ===
						pick
							? 'bg-accent'
							: ''}"
						onmousedown={(e) => e.preventDefault()}
						onclick={() => useSkill(s)}
					>
						<span class="flex w-full items-baseline gap-2">
							<span class="font-medium">/{s.command}</span>
							{#if s.scope !== 'user'}
								<span class="ml-auto text-xs text-muted-foreground">{s.scope}</span>
							{/if}
						</span>
						{#if s.description}
							<span class="line-clamp-1 text-xs text-muted-foreground">{s.description}</span>
						{/if}
					</button>
				</li>
			{/each}
		</ul>
	{/if}
	{#if !expanded}
		{@render toolbar(false)}
	{/if}
</form>

<Dialog.Root
	bind:open={expanded}
	onOpenChange={(open) => {
		if (!open) requestAnimationFrame(() => editor?.focus());
	}}
>
	<Dialog.Content
		class="flex h-[min(92dvh,70rem)] w-[min(96vw,80rem)] max-w-none flex-col gap-2 p-3 max-sm:h-dvh max-sm:w-screen max-sm:max-w-none max-sm:rounded-none sm:max-w-none sm:p-4"
		showCloseButton={false}
		escapeKeydownBehavior={vimOn && focused ? 'ignore' : 'close'}
	>
		<Dialog.Title class="sr-only">{label}</Dialog.Title>
		<Dialog.Description class="sr-only">The prompt, with room to write.</Dialog.Description>
		{#if preview}
			<div class="min-h-0 flex-1 overflow-auto rounded-md border bg-card p-4 text-sm">
				<div class="prose-chat">
					<!-- eslint-disable-next-line svelte/no-at-html-tags -->
					{@html rendered}
				</div>
			</div>
		{/if}
		<div
			bind:this={bigHost}
			class="{frameClass} flex min-h-0 flex-1 flex-col {preview ? 'hidden' : ''} {locked
				? 'opacity-60'
				: ''}"
		></div>
		<form
			class="flex flex-col gap-1.5"
			onsubmit={(e) => {
				e.preventDefault();
				onsubmit();
			}}
		>
			{@render toolbar(true)}
		</form>
	</Dialog.Content>
</Dialog.Root>

<Sheet.Root bind:open={historyOpen}>
	<Sheet.Content
		side={touch() ? 'bottom' : 'right'}
		class="flex max-h-[85vh] flex-col gap-2 p-4 sm:max-w-md"
	>
		<Sheet.Header class="p-0">
			<Sheet.Title>Prompt history</Sheet.Title>
			<Sheet.Description>{historyNote}</Sheet.Description>
		</Sheet.Header>
		<Input bind:value={historyQuery} placeholder="Search…" aria-label="Search prompts" />
		<ul class="min-h-0 flex-1 divide-y overflow-auto text-sm">
			{#each filtered as p (p.at + p.session)}
				<li>
					<button
						type="button"
						class="flex w-full flex-col items-start gap-0.5 px-1 py-2 text-left hover:bg-accent"
						onclick={() => pickHistory(p)}
					>
						<span class="line-clamp-3 whitespace-pre-wrap">{p.text}</span>
						<span class="text-xs text-muted-foreground"
							>{fmtStamp(p.at)}{session && p.session !== session ? ' · another session' : ''}</span
						>
					</button>
				</li>
			{:else}
				<li class="py-2 text-muted-foreground">
					{history.length ? 'No match.' : 'No prompts yet.'}
				</li>
			{/each}
		</ul>
	</Sheet.Content>
</Sheet.Root>
