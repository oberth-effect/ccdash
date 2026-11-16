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
	// A prompt: one frame around the editor (`Editor`, CodeMirror with vim
	// keys when on) and a single row of buttons, shared by the session
	// composer and the start sheet. The row holds only what is used all the
	// time: `+` (a photo or a file, templates, a note link, a skill, the
	// editor on a machine, the keys), `Aa` (the formatting bar above the
	// text, with the preview; off until asked for, kept per device), the
	// prompt history (`PromptHistory`) and Expand; the caller's buttons end
	// it. Enter is always a new line; Ctrl+Enter submits, Ctrl+Shift+Enter
	// takes the other way (Next prompt). Ctrl+↑/↓ walk the prompt history
	// (Ctrl+R searches it), Ctrl+G opens the text in an editor on a machine
	// (`onedit`), Ctrl+Shift+F expands the field to most of the screen. A
	// message that starts with `/` offers the skills of the field's host and
	// workspace, as the terminal does; arrows, Enter, Tab or a tap complete
	// one. Every key is in the panel's help (`help.svelte.ts`). With a
	// `tray`, files are attached from the menu, by pasting or by dropping
	// them on the field; they wait above the buttons until the prompt goes.
	// The microphone dictates: it records, a machine puts the recording into
	// words (`dictation.svelte.ts`), and they land at the cursor to read over.
	import { onDestroy, type Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import ALargeSmallIcon from '@lucide/svelte/icons/a-large-small';
	import BoldIcon from '@lucide/svelte/icons/bold';
	import CameraIcon from '@lucide/svelte/icons/camera';
	import CodeIcon from '@lucide/svelte/icons/code';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import FileIcon from '@lucide/svelte/icons/file';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import HeadingIcon from '@lucide/svelte/icons/heading';
	import HistoryIcon from '@lucide/svelte/icons/history';
	import ItalicIcon from '@lucide/svelte/icons/italic';
	import KeyboardIcon from '@lucide/svelte/icons/keyboard';
	import LinkIcon from '@lucide/svelte/icons/link';
	import ListIcon from '@lucide/svelte/icons/list';
	import ListOrderedIcon from '@lucide/svelte/icons/list-ordered';
	import LoaderCircleIcon from '@lucide/svelte/icons/loader-circle';
	import Maximize2Icon from '@lucide/svelte/icons/maximize-2';
	import MicIcon from '@lucide/svelte/icons/mic';
	import Minimize2Icon from '@lucide/svelte/icons/minimize-2';
	import NotebookPenIcon from '@lucide/svelte/icons/notebook-pen';
	import PaperclipIcon from '@lucide/svelte/icons/paperclip';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import SquareCodeIcon from '@lucide/svelte/icons/square-code';
	import SquareSlashIcon from '@lucide/svelte/icons/square-slash';
	import SquareTerminalIcon from '@lucide/svelte/icons/square-terminal';
	import StrikethroughIcon from '@lucide/svelte/icons/strikethrough';
	import TextQuoteIcon from '@lucide/svelte/icons/text-quote';
	import XIcon from '@lucide/svelte/icons/x';
	import { marked } from 'marked';
	import { Unauthorized } from '#lib/api/client.js';
	import { fetchSkills, type Editing, type Prompt } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import type { Tray } from '#lib/attachments.svelte.js';
	import { Dictation, MAX_SECONDS, dictationHostFor } from '#lib/dictation.svelte.js';
	import { fmtBytes, fmtDuration } from '#lib/format.js';
	import { help } from '#lib/help.svelte.js';
	import { live } from '#lib/live/store.svelte.js';
	import { fence, link, prefixLines, wrap, type Edit } from '#lib/mdformat.js';
	import { palette } from '#lib/palette.svelte.js';
	import { prefs } from '#lib/prefs.svelte.js';
	import { modeLabel } from '#lib/vim.js';
	import Editor from './Editor.svelte';
	import PromptHistory from './PromptHistory.svelte';

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
		tray = null,
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
		/** The history, loaded on first use (duplicates are dropped); with a
		 * query, the prompts that hold every word of it. */
		prompts: (query?: string) => Promise<Prompt[]>;
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
		/** The files the prompt carries; null where attaching is off (the hub
		 * or the session's machine does not do it). */
		tray?: Tray | null;
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
	let fileInput = $state<HTMLInputElement | null>(null);
	let cameraInput = $state<HTMLInputElement | null>(null);
	/** A file is held over the field. */
	let dropping = $state(false);
	// svelte-ignore state_referenced_locally
	const dictation = new Dictation(
		session,
		() => host,
		(words) => {
			preview = false;
			// NOTE: a space between what was there and what was said, none doubled
			const { from } = editor?.selection() ?? { from: 0 };
			const lead = from > 0 && !/\s/.test(text[from - 1] ?? ' ') ? ' ' : '';
			editor?.insert(`${lead}${words} `);
		}
	);
	onDestroy(() => dictation.destroy());
	/** The machine that would put a recording into words; null: none can. */
	const dictateHost = $derived(dictationHostFor(host));
	const clock = (seconds: number) =>
		`${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
	let history = $state<Prompt[]>([]);
	let historyLoaded = false;
	/** Where up/down is in `history`; -1 is the field's own text. */
	let cursor = $state(-1);
	let stash = '';

	let skills = $state<Skill[] | null>(null);
	/** Escape closed the menu; typing opens it again. */
	let dismissed = $state(false);
	let pick = $state(0);

	const vimOn = $derived(prefs.vimOn);
	const locked = $derived(disabled || Boolean(editing));
	const chars = $derived(text.length);
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

	type Format =
		| 'bold'
		| 'italic'
		| 'strike'
		| 'code'
		| 'block'
		| 'link'
		| 'bullet'
		| 'number'
		| 'quote'
		| 'heading';

	const FORMATS: Record<Format, (text: string, from: number, to: number) => Edit> = {
		bold: (t, a, b) => wrap(t, a, b, '**'),
		italic: (t, a, b) => wrap(t, a, b, '_'),
		strike: (t, a, b) => wrap(t, a, b, '~~'),
		code: (t, a, b) => wrap(t, a, b, '`'),
		block: fence,
		link,
		bullet: (t, a, b) => prefixLines(t, a, b, 'bullet'),
		number: (t, a, b) => prefixLines(t, a, b, 'number'),
		quote: (t, a, b) => prefixLines(t, a, b, 'quote'),
		heading: (t, a, b) => prefixLines(t, a, b, 'heading')
	};

	/** One formatting step on the selection, undone as one. */
	function format(kind: Format) {
		if (!editor || locked) return;
		preview = false;
		const { from, to } = editor.selection();
		const edit = FORMATS[kind](text, from, to);
		editor.replaceRange(edit.from, edit.to, edit.insert, edit.select);
	}

	function picked(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		if (input.files?.length) tray?.add(input.files);
		// NOTE: the same file picked again must fire `change` again
		input.value = '';
	}

	/** A screenshot or a copied file pasted into the field. */
	function onpaste(event: ClipboardEvent) {
		const files = event.clipboardData?.files;
		if (!tray || locked || !files?.length) return;
		event.preventDefault();
		event.stopPropagation();
		tray.add(files);
	}

	const carriesFiles = (event: DragEvent) =>
		Boolean(tray && !locked && event.dataTransfer?.types.includes('Files'));

	function ondragover(event: DragEvent) {
		if (!carriesFiles(event)) return;
		event.preventDefault();
		dropping = true;
	}

	function ondrop(event: DragEvent) {
		if (!carriesFiles(event)) return;
		event.preventDefault();
		event.stopPropagation();
		dropping = false;
		if (event.dataTransfer?.files.length) tray?.add(event.dataTransfer.files);
	}

	/** A skill from the menu: the field is empty, so `/` opens the list. */
	function startSkill() {
		dismissed = false;
		editor?.replaceRange(0, text.length, '/');
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
			if (event.code === 'KeyD') {
				if (!dictateHost) return false;
				if (dictation.state === 'recording') dictation.done();
				else if (!locked) dictation.start();
				return true;
			}
			const digit = /^Digit([1-9])$/.exec(event.code);
			if (!digit || !onquick) return false;
			onquick(Number(digit[1]) - 1);
			return true;
		}
		if (key === 'e') format('block');
		else if (key === 'b' || key === 'i') {
			// NOTE: vim's normal mode has its own Ctrl B and Ctrl I
			if (vimOn && mode !== 'insert') return false;
			format(key === 'b' ? 'bold' : 'italic');
		} else if (key === 'p') preview = !preview;
		else if (key === 'g') {
			if (!onedit || !editHost || locked) return false;
			onedit();
		} else if (key === 'r') openHistory();
		else if (key === 'j') {
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

	/** A prompt from the history: instead of the text, or at the cursor.
	 * NOTE: through the editor, so undo brings the draft back. */
	function takeHistory(p: Prompt, how: 'use' | 'insert') {
		cursor = -1;
		historyOpen = false;
		preview = false;
		requestAnimationFrame(() => {
			if (how === 'use') editor?.replaceRange(0, text.length, p.text);
			else editor?.insert(p.text);
		});
	}

	function openHistory() {
		historyOpen = true;
		loadHistory();
	}

	/** NOTE: a tap on a formatting button must not take the focus (and the
	 * phone's keyboard) from the text. */
	const keepFocus = (event: MouseEvent) => event.preventDefault();

	/** `:x` and `:q`: done here, back to the page (or out of Expand). */
	function quit() {
		if (expanded) expanded = false;
		else (document.activeElement as HTMLElement | null)?.blur();
	}

	/** Why attaching is off, for the menu's disabled entry. */
	const attachOff = $derived(
		live.hub?.features.attachments
			? host
				? `${host}'s collector does not fetch attachments ([attachments] enable, hub.token_file)`
				: 'Pick a machine first'
			: 'Attachments are off on the hub ([attachments] enable)'
	);
	const frameClass =
		'flex min-h-0 flex-col rounded-lg border bg-background transition-colors focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/40';
	const FORMAT_BUTTONS = [
		['bold', 'Bold (Ctrl B)', BoldIcon],
		['italic', 'Italic (Ctrl I)', ItalicIcon],
		['strike', 'Strikethrough', StrikethroughIcon],
		['code', 'Code', CodeIcon],
		['block', 'Code block (Ctrl E)', SquareCodeIcon],
		['link', 'Link', LinkIcon],
		['bullet', 'Bulleted list', ListIcon],
		['number', 'Numbered list', ListOrderedIcon],
		['quote', 'Quote', TextQuoteIcon],
		['heading', 'Heading', HeadingIcon]
	] as const;
</script>

<svelte:window onkeydown={onwindowkey} />

{#snippet formatbar()}
	<div
		class="flex items-center gap-0.5 overflow-x-auto border-b px-1.5 py-1"
		role="toolbar"
		aria-label="Formatting"
	>
		{#each FORMAT_BUTTONS as [kind, title, Icon], i (kind)}
			{#if i === 5}<span class="mx-1 h-4 w-px shrink-0 bg-border" aria-hidden="true"></span>{/if}
			<Button
				variant="ghost"
				size="icon-sm"
				type="button"
				{title}
				aria-label={title}
				disabled={locked}
				onmousedown={keepFocus}
				onclick={() => format(kind)}
			>
				<Icon />
			</Button>
		{/each}
		<Button
			variant={preview ? 'secondary' : 'ghost'}
			size="sm"
			type="button"
			class="ml-auto shrink-0"
			title="Preview the markdown (Ctrl P)"
			aria-label="Preview"
			aria-pressed={preview}
			onclick={() => (preview = !preview)}
		>
			<EyeIcon /> <span class="max-sm:hidden">Preview</span>
		</Button>
	</div>
{/snippet}

{#snippet toolbar(big: boolean)}
	<div data-composer-bar class="flex items-center gap-1 px-1.5 pb-1.5 {editing ? 'flex-wrap' : ''}">
		{#if editing}
			<span class="flex items-center gap-2 px-1 text-xs text-muted-foreground" role="status">
				<SquareTerminalIcon class="size-3.5 shrink-0 text-working" />
				In the editor on {editing.host} · {fmtDuration(live.now - editing.since)} · the text lands here
				when it closes
				{#if oncanceledit}
					<Button variant="outline" size="xs" type="button" onclick={oncanceledit}>Cancel</Button>
				{/if}
			</span>
		{:else if dictation.busy}
			<span class="flex h-7 min-w-0 items-center gap-2 px-1 text-xs" role="status">
				{#if dictation.state === 'recording'}
					<span class="size-2.5 shrink-0 animate-pulse rounded-full bg-critical" aria-hidden="true"
					></span>
					<span class="shrink-0 font-mono whitespace-nowrap tabular-nums"
						>{clock(dictation.seconds)}</span
					>
					<span
						class="h-1.5 w-12 shrink-0 overflow-hidden rounded-full bg-muted sm:w-20"
						aria-hidden="true"
					>
						<span
							class="block h-full rounded-full bg-working"
							style:width="{Math.min(100, dictation.level * 140)}%"
						></span>
					</span>
					{#if dictation.seconds >= MAX_SECONDS - 30}
						<span class="text-warning">stops at {clock(MAX_SECONDS)}</span>
					{/if}
					<Button size="xs" type="button" onclick={() => dictation.done()}>Done</Button>
				{:else if dictation.state === 'working'}
					<LoaderCircleIcon class="size-3.5 shrink-0 animate-spin text-working" />
					<span class="truncate text-muted-foreground"
						>{dictation.host} is putting it into words…</span
					>
				{:else}
					<MicIcon class="size-3.5 shrink-0 text-muted-foreground" />
					<span class="truncate text-muted-foreground">Allow the microphone…</span>
				{/if}
				<Button variant="ghost" size="xs" type="button" onclick={() => dictation.cancel()}
					>Cancel</Button
				>
			</span>
		{:else}
			<DropdownMenu.Root>
				<DropdownMenu.Trigger>
					{#snippet child({ props })}
						<Button
							variant="ghost"
							size="icon-sm"
							type="button"
							title="Add: a template, a note link, a skill…"
							aria-label="Add to the prompt"
							{...props}
						>
							<PlusIcon />
						</Button>
					{/snippet}
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="start" side="top" class="w-64">
					{#if prefs.templates.length}
						<DropdownMenu.Sub>
							<DropdownMenu.SubTrigger disabled={locked}>
								<FileTextIcon /> Template
							</DropdownMenu.SubTrigger>
							<DropdownMenu.SubContent class="max-h-72 w-56 overflow-y-auto">
								{#each prefs.templates as t (t.name)}
									<DropdownMenu.Item onSelect={() => insert(t.text)}>{t.name}</DropdownMenu.Item>
								{/each}
								<DropdownMenu.Separator />
								<DropdownMenu.Item onSelect={() => goto('/settings/composer')}>
									Edit the templates…
								</DropdownMenu.Item>
							</DropdownMenu.SubContent>
						</DropdownMenu.Sub>
					{:else}
						<DropdownMenu.Item onSelect={() => goto('/settings/composer')}>
							<FileTextIcon /> Template
							<span class="ml-auto pl-2 text-xs text-muted-foreground">none yet: add one</span>
						</DropdownMenu.Item>
					{/if}
					<DropdownMenu.Item disabled={locked} onSelect={noteLink}>
						<NotebookPenIcon /> Link to a note
						<DropdownMenu.Shortcut>[[</DropdownMenu.Shortcut>
					</DropdownMenu.Item>
					{#if host}
						<DropdownMenu.Item disabled={locked || text.length > 0} onSelect={startSkill}>
							<SquareSlashIcon /> Skill
							<span class="ml-auto pl-2 text-xs text-muted-foreground"
								>{text.length ? 'only starts a prompt' : '/'}</span
							>
						</DropdownMenu.Item>
					{/if}
					{#if onedit}
						<DropdownMenu.Separator />
						<DropdownMenu.Item
							disabled={locked || !editHost}
							title={editHost
								? undefined
								: 'No connected machine opens an editor: set [edit] command in its collector'}
							onSelect={onedit}
						>
							<SquareTerminalIcon />
							Edit in your editor
							<span class="ml-auto pl-2 text-xs text-muted-foreground"
								>{editHost ? `on ${editHost}` : 'no machine can'}</span
							>
						</DropdownMenu.Item>
					{/if}
					<DropdownMenu.Separator />
					{#if tray}
						<DropdownMenu.Item disabled={locked} onSelect={() => fileInput?.click()}>
							<PaperclipIcon /> Photo or file
							<span class="ml-auto pl-2 text-xs text-muted-foreground">or paste, or drop</span>
						</DropdownMenu.Item>
						<DropdownMenu.Item
							class="pointer-fine:hidden"
							disabled={locked}
							onSelect={() => cameraInput?.click()}
						>
							<CameraIcon /> Take a photo
						</DropdownMenu.Item>
						<DropdownMenu.CheckboxItem
							checked={!prefs.attach.resize}
							closeOnSelect={false}
							onCheckedChange={(full) => prefs.setAttach({ resize: !full })}
						>
							Photos in full size
						</DropdownMenu.CheckboxItem>
					{:else}
						<DropdownMenu.Item disabled title={attachOff}>
							<PaperclipIcon /> Photo or file
							<span class="ml-auto pl-2 text-xs text-muted-foreground">off here</span>
						</DropdownMenu.Item>
					{/if}
					<DropdownMenu.Item
						disabled={locked || !dictateHost}
						title={dictateHost
							? undefined
							: 'No connected machine puts recordings into words: set [transcribe] command in its collector'}
						onSelect={() => dictation.start()}
					>
						<MicIcon /> Dictate
						<span class="ml-auto pl-2 text-xs text-muted-foreground"
							>{dictateHost ? `words by ${dictateHost}` : 'no machine can'}</span
						>
					</DropdownMenu.Item>
					{#if dictateHost && expandable && !fill}
						<DropdownMenu.Item class="sm:hidden" onSelect={toggleExpand}>
							{#if big}<Minimize2Icon /> Back to the page{:else}<Maximize2Icon /> Expand the field{/if}
						</DropdownMenu.Item>
					{/if}
					<DropdownMenu.Separator />
					<DropdownMenu.Item onSelect={() => help.show()}>
						<KeyboardIcon /> Keys and help
						<DropdownMenu.Shortcut>?</DropdownMenu.Shortcut>
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
			<Button
				variant={prefs.formatBar ? 'secondary' : 'ghost'}
				size="icon-sm"
				type="button"
				title={prefs.formatBar ? 'Hide the formatting bar' : 'Formatting and preview'}
				aria-label="Formatting bar"
				aria-pressed={prefs.formatBar}
				onmousedown={keepFocus}
				onclick={() => {
					prefs.setFormatBar(!prefs.formatBar);
					if (!prefs.formatBar) preview = false;
				}}
			>
				<ALargeSmallIcon />
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				type="button"
				title="Prompt history (Ctrl R)"
				aria-label="Prompt history"
				onclick={openHistory}
			>
				<HistoryIcon />
			</Button>
			{#if dictateHost}
				<Button
					variant="ghost"
					size="icon-sm"
					type="button"
					title="Dictate: record, and {dictateHost} puts it into words here"
					aria-label="Dictate"
					disabled={locked}
					onclick={() => dictation.start()}
				>
					<MicIcon />
				</Button>
			{/if}
			{#if expandable && !fill}
				<Button
					variant="ghost"
					size="icon-sm"
					type="button"
					class={dictateHost ? 'max-sm:hidden' : ''}
					title={big ? 'Back to the page (Ctrl Shift F)' : 'Expand the field (Ctrl Shift F)'}
					aria-label={big ? 'Shrink the field' : 'Expand the field'}
					onclick={toggleExpand}
				>
					{#if big}<Minimize2Icon />{:else}<Maximize2Icon />{/if}
				</Button>
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
		<!-- NOTE: on a phone the recording takes the row: Send comes back with the words -->
		<span
			class="ml-auto flex shrink-0 items-center gap-1.5 pl-1 {dictation.busy
				? 'max-sm:hidden'
				: ''}">{@render actions()}</span
		>
	</div>
{/snippet}

{#snippet chips()}
	<ul class="flex flex-wrap gap-1.5 px-1.5 pb-1.5" aria-label="Attached files">
		{#each tray?.items ?? [] as item (item.key)}
			<li
				class="relative flex max-w-full items-center gap-2 overflow-hidden rounded-md border bg-card py-1 pr-1 pl-1 text-xs {item.state ===
				'failed'
					? 'border-critical/50'
					: ''}"
			>
				{#if item.preview || (item.attachment?.kind === 'image' && item.attachment.url)}
					<img
						src={item.preview ?? item.attachment?.url}
						alt=""
						class="size-9 shrink-0 rounded object-cover"
					/>
				{:else}
					<span class="flex size-9 shrink-0 items-center justify-center rounded bg-muted">
						<FileIcon class="size-4 text-muted-foreground" />
					</span>
				{/if}
				<span class="flex min-w-0 flex-col">
					<span class="max-w-40 truncate font-medium">{item.name}</span>
					{#if item.state === 'failed'}
						<span class="max-w-56 text-critical" role="alert">{item.error}</span>
					{:else}
						<span class="text-muted-foreground">
							{fmtBytes(item.bytes)}{item.state === 'uploading'
								? ` · ${Math.round(item.progress * 100)}%`
								: ''}
						</span>
					{/if}
				</span>
				<Button
					variant="ghost"
					size="icon-xs"
					type="button"
					aria-label="Remove {item.name}"
					title={item.state === 'uploading' ? 'Cancel the upload' : 'Remove'}
					onclick={() => tray?.remove(item.key)}
				>
					<XIcon />
				</Button>
				{#if item.state === 'uploading'}
					<span
						class="absolute bottom-0 left-0 h-0.5 bg-working transition-[width]"
						style:width="{Math.max(4, item.progress * 100)}%"
						aria-hidden="true"
					></span>
				{/if}
			</li>
		{/each}
	</ul>
{/snippet}

{#snippet shown(big: boolean)}
	{#if preview}
		<div
			class="overflow-auto px-3 py-2 text-sm {big || fill ? 'min-h-0 flex-1' : 'max-h-56'}"
			aria-label="Preview"
		>
			{#if text.trim()}
				<div class="prose-chat">
					<!-- eslint-disable-next-line svelte/no-at-html-tags -->
					{@html rendered}
				</div>
			{:else}
				<p class="text-muted-foreground">Nothing to preview yet.</p>
			{/if}
		</div>
	{/if}
{/snippet}

<form
	class="flex min-h-0 flex-col gap-1.5 {fill ? 'flex-1' : ''}"
	onsubmit={(e) => {
		e.preventDefault();
		onsubmit();
	}}
>
	<!-- NOTE: capture: before the editor takes a pasted or dropped file as text -->
	<div
		class="{frameClass} {fill ? 'flex-1' : ''} {expanded ? 'hidden' : ''} {dropping
			? 'border-working ring-2 ring-working/40'
			: ''}"
		role="group"
		aria-label={label}
		onpastecapture={onpaste}
		ondragovercapture={ondragover}
		ondragleave={() => (dropping = false)}
		ondropcapture={ondrop}
	>
		{#if prefs.formatBar}{@render formatbar()}{/if}
		{#if !expanded}{@render shown(false)}{/if}
		<!-- NOTE: the editor's element moves into the expanded dialog and back; it is created once, here -->
		<div
			bind:this={inlineHost}
			class="{fill ? 'flex min-h-0 flex-1 flex-col' : ''} {preview ? 'hidden' : ''} {locked
				? 'opacity-60'
				: ''}"
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
				class="mx-1.5 mb-1.5 max-h-64 overflow-auto rounded-md border bg-popover text-sm shadow-md"
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
		{#if tray && tray.items.length && !expanded}{@render chips()}{/if}
		{#if !expanded}{@render toolbar(false)}{/if}
	</div>
	{#if tray}
		<input bind:this={fileInput} type="file" multiple class="hidden" onchange={picked} />
		<input
			bind:this={cameraInput}
			type="file"
			accept="image/*"
			capture="environment"
			class="hidden"
			onchange={picked}
		/>
		{#if tray.notice}<p class="text-xs text-warning" role="status">{tray.notice}</p>{/if}
	{/if}
	{#if dictation.error}
		<p class="text-xs text-warning" role="alert">Dictation: {dictation.error}</p>
	{/if}
	{#if notice && !editing}
		<p class="text-xs text-muted-foreground" role="status">Editor: {notice}</p>
	{/if}
	{#if vimrcErrors.length}
		<p class="text-xs text-warning" role="status">
			Vimrc lines not taken: {vimrcErrors.join('; ')}
			(<a class="underline" href="/settings/composer">Settings › Composer</a>)
		</p>
	{/if}
</form>

<Dialog.Root
	bind:open={expanded}
	onOpenChange={(open) => {
		if (!open) requestAnimationFrame(() => editor?.focus());
	}}
>
	<Dialog.Content
		class="flex h-[min(92dvh,70rem)] w-[min(96vw,80rem)] max-w-none flex-col gap-2 p-3 max-sm:h-dvh max-sm:w-screen max-sm:max-w-none max-sm:rounded-none max-sm:p-2 sm:max-w-none sm:p-4"
		showCloseButton={false}
		escapeKeydownBehavior={vimOn && focused ? 'ignore' : 'close'}
	>
		<Dialog.Title class="sr-only">{label}</Dialog.Title>
		<Dialog.Description class="sr-only">The prompt, with room to write.</Dialog.Description>
		<form
			class="{frameClass} flex-1"
			onsubmit={(e) => {
				e.preventDefault();
				onsubmit();
			}}
		>
			{#if prefs.formatBar}{@render formatbar()}{/if}
			{@render shown(true)}
			<div
				bind:this={bigHost}
				class="flex min-h-0 flex-1 flex-col {preview ? 'hidden' : ''} {locked ? 'opacity-60' : ''}"
			></div>
			{#if tray && tray.items.length}{@render chips()}{/if}
			{@render toolbar(true)}
		</form>
	</Dialog.Content>
</Dialog.Root>

<PromptHistory
	bind:open={historyOpen}
	recent={history}
	search={(query) => prompts(query)}
	{session}
	note={historyNote}
	onuse={(p) => takeHistory(p, 'use')}
	oninsert={(p) => takeHistory(p, 'insert')}
/>
