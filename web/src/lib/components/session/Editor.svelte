<script lang="ts">
	// The text editor under the composer and the start sheet: CodeMirror 6
	// with markdown, soft wrap, an undo history that survives what the
	// toolbar inserts, and the owner's vim keys when they want them
	// (`prefs.vim`, `prefs.vimrc`). Line numbers (absolute, or relative to
	// the cursor's line as Neovim's number + relativenumber), the markdown
	// highlighting and the wrapping are each a switch (`prefs.editor`).
	// Grows with its text to `maxHeight`, or fills its parent (`fill`). The
	// parent gets the raw keys first (`onkey`) for its shortcuts; what it
	// does not take goes to vim and the default bindings.
	import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
	import { markdown, markdownKeymap } from '@codemirror/lang-markdown';
	import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
	import { highlightSelectionMatches, searchKeymap } from '@codemirror/search';
	import { Compartment, EditorState, Prec, type Extension } from '@codemirror/state';
	import {
		EditorView,
		GutterMarker,
		drawSelection,
		gutter,
		keymap,
		placeholder as placeholderExt
	} from '@codemirror/view';
	import { tags } from '@lezer/highlight';
	import { getCM } from '@replit/codemirror-vim';
	import { onMount } from 'svelte';
	import { cancelHop, hopActive, hopExtension, hopKey, startHop } from '#lib/hop.js';
	import {
		applyVimrc,
		pasteEvent,
		pasteFromClipboard,
		setClipboard,
		setVimHooks,
		vimExtension
	} from '#lib/vim.js';

	let {
		value = $bindable(''),
		placeholder = '',
		label = 'Prompt',
		disabled = false,
		vim = false,
		vimrc = '',
		numbers = 'off',
		highlight = true,
		wrap = true,
		hop = true,
		hopKeys = 'asdfghjkl',
		clipboard = true,
		pasteReads = true,
		fill = false,
		rows = 1,
		maxHeight = 220,
		onkey,
		onfocus,
		onmode,
		onwrite,
		onquit,
		onvimrc
	}: {
		value?: string;
		placeholder?: string;
		label?: string;
		disabled?: boolean;
		/** Vim keys on; `vimrc` is applied when it turns on or changes. */
		vim?: boolean;
		vimrc?: string;
		/** Line numbers: none, absolute, or relative to the cursor's line. */
		numbers?: 'off' | 'absolute' | 'relative';
		/** Markdown set apart as it is typed. */
		highlight?: boolean;
		/** Long lines wrap; off, they scroll sideways. */
		wrap?: boolean;
		/** With vim: `,` then two characters jumps to a match in view. */
		hop?: boolean;
		hopKeys?: string;
		/** With vim: yanks go to the system clipboard, `p` reads it. */
		clipboard?: boolean;
		/** With vim: a bare `p` reads the clipboard (else the register). */
		pasteReads?: boolean;
		/** Fill the parent's height instead of growing to `maxHeight`. */
		fill?: boolean;
		/** Lines the empty editor shows. */
		rows?: number;
		maxHeight?: number;
		/** A key before vim and the bindings; true takes it. */
		onkey?: (event: KeyboardEvent) => boolean;
		onfocus?: (focused: boolean) => void;
		/** The vim mode changed (`normal`, `insert`, `visual` + sub-mode, `replace`). */
		onmode?: (mode: string, subMode?: string) => void;
		/** `:w` */
		onwrite?: () => void;
		/** `:x`, `:wq`, `:q` */
		onquit?: () => void;
		/** The vimrc lines vim did not take. */
		onvimrc?: (errors: string[]) => void;
	} = $props();

	let root = $state<HTMLDivElement | null>(null);
	let view: EditorView | null = null;
	/** The mode listener is on the adapter already. */
	let armed = false;
	const vimComp = new Compartment();
	const editable = new Compartment();
	const hint = new Compartment();
	const numbersComp = new Compartment();
	const highlightComp = new Compartment();
	const wrapComp = new Compartment();

	const theme = EditorView.theme({
		'&': { fontSize: '0.875rem', backgroundColor: 'transparent', color: 'var(--foreground)' },
		'&.cm-focused': { outline: 'none' },
		'.cm-scroller': { fontFamily: 'inherit', lineHeight: '1.5', overflow: 'auto' },
		'.cm-content': { padding: '0.5rem 0', caretColor: 'var(--foreground)' },
		'.cm-line': { padding: '0 0.75rem' },
		'.cm-placeholder': { color: 'var(--muted-foreground)' },
		'.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--foreground)' },
		'.cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground':
			{ backgroundColor: 'color-mix(in oklab, var(--primary) 22%, transparent)' },
		'.cm-fat-cursor': {
			backgroundColor: 'var(--primary)',
			color: 'var(--primary-foreground) !important'
		},
		'&:not(.cm-focused) .cm-fat-cursor': {
			backgroundColor: 'transparent',
			outline: 'solid 1px var(--primary)',
			color: 'inherit !important'
		},
		'.cm-searchMatch': { backgroundColor: 'color-mix(in oklab, var(--warning) 35%, transparent)' },
		'.cm-searchMatch.cm-searchMatch-selected': {
			backgroundColor: 'color-mix(in oklab, var(--warning) 60%, transparent)'
		},
		'.cm-selectionMatch': {
			backgroundColor: 'color-mix(in oklab, var(--primary) 12%, transparent)'
		},
		'.cm-gutters': {
			backgroundColor: 'transparent',
			borderRight: '1px solid var(--border)',
			color: 'var(--muted-foreground)',
			fontFamily: 'var(--font-mono)',
			fontSize: '0.75rem'
		},
		'.cm-gutter.cm-lineNumbers .cm-gutterElement': {
			minWidth: '2.6em',
			padding: '0 0.5rem',
			textAlign: 'right'
		},
		'.cm-gutterElement.cm-numberCurrent': { color: 'var(--foreground)' },
		'.cm-vim-panel': {
			fontFamily: 'var(--font-mono)',
			fontSize: '0.75rem',
			color: 'var(--muted-foreground)',
			padding: '0 0.75rem'
		},
		'.cm-vim-panel input': { color: 'var(--foreground)' }
	});
	// markdown as it reads, not a code theme: the marks step back
	const style = HighlightStyle.define([
		{ tag: tags.heading, fontWeight: '600' },
		{ tag: tags.strong, fontWeight: '600' },
		{ tag: tags.emphasis, fontStyle: 'italic' },
		{ tag: tags.strikethrough, textDecoration: 'line-through' },
		{ tag: tags.link, textDecoration: 'underline', textUnderlineOffset: '2px' },
		{ tag: tags.url, color: 'var(--muted-foreground)' },
		{ tag: tags.monospace, fontFamily: 'var(--font-mono)', fontSize: '0.92em' },
		{ tag: tags.quote, color: 'var(--muted-foreground)' },
		{
			tag: [tags.processingInstruction, tags.meta, tags.labelName],
			color: 'var(--muted-foreground)'
		}
	]);

	class Num extends GutterMarker {
		text: string;
		current: boolean;
		constructor(text: string, current: boolean) {
			super();
			this.text = text;
			this.current = current;
			this.elementClass = current ? 'cm-numberCurrent' : '';
		}
		eq(other: Num) {
			return other.text === this.text && other.current === this.current;
		}
		toDOM() {
			return document.createTextNode(this.text);
		}
	}

	/** A gutter of line numbers; relative ones count from the cursor's
	 * line, which shows its own number (Neovim's hybrid). */
	function numbersExt(mode: 'off' | 'absolute' | 'relative'): Extension {
		if (mode === 'off') return [];
		return gutter({
			class: 'cm-lineNumbers',
			lineMarker(v, line) {
				const n = v.state.doc.lineAt(line.from).number;
				const cur = v.state.doc.lineAt(v.state.selection.main.head).number;
				const shown = mode === 'relative' && n !== cur ? Math.abs(n - cur) : n;
				return new Num(String(shown), n === cur);
			},
			lineMarkerChange: (u) => u.selectionSet || u.docChanged || u.viewportChanged,
			initialSpacer: (v) => new Num(String(Math.max(99, v.state.doc.lines)), false)
		});
	}

	const highlightExt = (on: boolean): Extension => (on ? syntaxHighlighting(style) : []);
	const wrapExt = (on: boolean): Extension => (on ? EditorView.lineWrapping : []);

	function create(parent: HTMLElement) {
		const state = EditorState.create({
			doc: value,
			extensions: [
				// NOTE: vim first: it must see keys before the other bindings
				vimComp.of(vim ? vimExtension() : []),
				Prec.highest(
					EditorView.domEventHandlers({
						keydown: (event, v) => keydown(event, v),
						paste: (event, v) => {
							if (!vim || !clipboard || !pasteEvent(v, event)) return false;
							event.preventDefault();
							return true;
						}
					})
				),
				editable.of([EditorView.editable.of(!disabled), EditorState.readOnly.of(disabled)]),
				hint.of(placeholderExt(placeholder)),
				numbersComp.of(numbersExt(numbers)),
				hopExtension(),
				history(),
				drawSelection(),
				wrapComp.of(wrapExt(wrap)),
				markdown(),
				highlightComp.of(highlightExt(highlight)),
				highlightSelectionMatches(),
				keymap.of([...markdownKeymap, ...historyKeymap, ...searchKeymap, ...defaultKeymap]),
				theme,
				EditorView.contentAttributes.of({ 'aria-label': label }),
				EditorView.updateListener.of((u) => {
					if (u.docChanged) {
						const text = u.state.doc.toString();
						if (text !== value) value = text;
					}
					if (u.focusChanged) {
						if (!u.view.hasFocus) cancelHop(u.view);
						onfocus?.(u.view.hasFocus);
					}
				})
			]
		});
		view = new EditorView({ state, parent });
		if (vim) armVim();
	}

	/** What the vim engine keeps per editor, as far as the keys here need. */
	type VimState = {
		insertMode: boolean;
		visualMode: boolean;
		inputState: {
			keyBuffer: string[];
			registerName?: string;
			operator: string | null;
			prefixRepeat: string[];
		};
	};

	/** The vim engine's state of this editor, when vim is on. */
	function vimState(v: EditorView): VimState | undefined {
		return getCM(v)?.state.vim as VimState | undefined;
	}

	/** The parent's shortcuts first, then a hop in progress, then `,`
	 * starting one, then a `p` reading the clipboard; the rest is vim's
	 * and the bindings'. */
	function keydown(event: KeyboardEvent, v: EditorView): boolean {
		if (onkey?.(event)) return true;
		if (!vim) return false;
		if (hopActive(v)) {
			if (hopKey(v, event, hopKeys)) {
				event.preventDefault();
				return true;
			}
			return false;
		}
		const state = vimState(v);
		if (!state || state.insertMode) return false;
		const idle =
			!state.inputState.keyBuffer.length &&
			!state.inputState.operator &&
			!state.inputState.registerName &&
			!state.inputState.prefixRepeat.length;
		if (hop && idle && event.key === ',' && !event.ctrlKey && !event.metaKey && !event.altKey) {
			startHop(v, state.visualMode);
			event.preventDefault();
			return true;
		}
		if (clipboard && pasteFromClipboard(v, event)) {
			event.preventDefault();
			return true;
		}
		return false;
	}

	/** After vim turned on: the mode events and the vimrc. */
	function armVim() {
		if (!view) return;
		setVimHooks(view, { onwrite, onquit });
		if (!armed) {
			armed = true;
			getCM(view)?.on('vim-mode-change', (e: { mode: string; subMode?: string }) =>
				onmode?.(e.mode, e.subMode)
			);
		}
		onvimrc?.(applyVimrc(view, vimrc));
		onmode?.('normal');
	}

	onMount(() => {
		if (root) create(root);
		return () => {
			view?.destroy();
			view = null;
		};
	});

	// text set from outside (history, a template, the hub, an editor) lands
	// as one undoable change
	$effect(() => {
		const text = value;
		if (!view || view.state.doc.toString() === text) return;
		view.dispatch({
			changes: { from: 0, to: view.state.doc.length, insert: text },
			selection: { anchor: text.length }
		});
	});

	$effect(() => {
		const on = vim;
		const rc = vimrc;
		void rc;
		if (!view) return;
		view.dispatch({ effects: vimComp.reconfigure(on ? vimExtension() : []) });
		if (on) armVim();
		else onmode?.('');
	});

	$effect(() => {
		const off = disabled;
		if (!view) return;
		view.dispatch({
			effects: editable.reconfigure([EditorView.editable.of(!off), EditorState.readOnly.of(off)])
		});
	});

	$effect(() => {
		const text = placeholder;
		if (!view) return;
		view.dispatch({ effects: hint.reconfigure(placeholderExt(text)) });
	});

	$effect(() => {
		const mode = numbers;
		if (!view) return;
		view.dispatch({ effects: numbersComp.reconfigure(numbersExt(mode)) });
	});

	$effect(() => {
		const on = highlight;
		if (!view) return;
		view.dispatch({ effects: highlightComp.reconfigure(highlightExt(on)) });
	});

	$effect(() => {
		const on = wrap;
		if (!view) return;
		view.dispatch({ effects: wrapComp.reconfigure(wrapExt(on)) });
	});

	$effect(() => {
		setClipboard(vim && clipboard, pasteReads);
	});

	$effect(() => {
		if (!hop && view) cancelHop(view);
	});

	$effect(() => {
		if (view) setVimHooks(view, { onwrite, onquit });
	});

	export function focus() {
		view?.focus();
	}

	export function hasFocus(): boolean {
		return view?.hasFocus ?? false;
	}

	/** The root element, for a parent that moves the editor elsewhere. */
	export function element(): HTMLElement | null {
		return root;
	}

	/** After the editor moved in the DOM. */
	export function measure() {
		view?.requestMeasure();
	}

	export function selection(): { from: number; to: number } {
		const r = view?.state.selection.main;
		return { from: r?.from ?? value.length, to: r?.to ?? value.length };
	}

	/** Replaces `from..to` with `text`, selecting `select` (offsets from `from`)
	 * or placing the cursor after it. */
	export function replaceRange(from: number, to: number, text: string, select?: [number, number]) {
		if (!view) return;
		const anchor = from + (select?.[0] ?? text.length);
		const head = from + (select?.[1] ?? text.length);
		view.dispatch({
			changes: { from, to, insert: text },
			selection: { anchor, head },
			scrollIntoView: true
		});
		view.focus();
	}

	/** Inserts at the selection. */
	export function insert(text: string, select?: [number, number]) {
		const { from, to } = selection();
		replaceRange(from, to, text, select);
	}
</script>

<div
	bind:this={root}
	class="editor {fill ? 'fill' : ''}"
	style:--rows={rows}
	style:--max-height="{maxHeight}px"
></div>

<style>
	.editor {
		display: flex;
		flex-direction: column;
		min-height: 0;
		min-width: 0;
	}
	.editor.fill {
		flex: 1 1 auto;
		height: 100%;
	}
	.editor :global(.cm-editor) {
		flex: 1 1 auto;
		min-height: 0;
		max-height: var(--max-height);
	}
	.editor.fill :global(.cm-editor) {
		max-height: none;
		height: 100%;
	}
	.editor :global(.cm-content) {
		min-height: calc(var(--rows) * 1.5em);
	}
</style>
