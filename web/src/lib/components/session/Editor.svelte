<script lang="ts">
	// The text editor under the composer and the start sheet: CodeMirror 6
	// with markdown, soft wrap, an undo history that survives what the
	// toolbar inserts, and the owner's vim keys when they want them
	// (`prefs.vim`, `prefs.vimrc`). Grows with its text to `maxHeight`, or
	// fills its parent (`fill`). The parent gets the raw keys first
	// (`onkey`) for its shortcuts; what it does not take goes to vim and
	// the default bindings.
	import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
	import { markdown, markdownKeymap } from '@codemirror/lang-markdown';
	import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
	import { highlightSelectionMatches, searchKeymap } from '@codemirror/search';
	import { Compartment, EditorState, Prec } from '@codemirror/state';
	import {
		EditorView,
		drawSelection,
		keymap,
		placeholder as placeholderExt
	} from '@codemirror/view';
	import { tags } from '@lezer/highlight';
	import { getCM } from '@replit/codemirror-vim';
	import { onMount } from 'svelte';
	import { applyVimrc, setVimHooks, vimExtension } from '#lib/vim.js';

	let {
		value = $bindable(''),
		placeholder = '',
		label = 'Prompt',
		disabled = false,
		vim = false,
		vimrc = '',
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
		'.cm-vim-panel': {
			fontFamily: 'var(--font-mono)',
			fontSize: '0.75rem',
			color: 'var(--muted-foreground)',
			padding: '0 0.75rem'
		},
		'.cm-vim-panel input': { color: 'var(--foreground)' }
	});
	// markdown as it reads, not a code theme: the marks step back
	const highlight = HighlightStyle.define([
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

	function create(parent: HTMLElement) {
		const state = EditorState.create({
			doc: value,
			extensions: [
				// NOTE: vim first: it must see keys before the other bindings
				vimComp.of(vim ? vimExtension() : []),
				Prec.highest(
					EditorView.domEventHandlers({
						keydown: (event) => (onkey ? onkey(event) : false)
					})
				),
				editable.of([EditorView.editable.of(!disabled), EditorState.readOnly.of(disabled)]),
				hint.of(placeholderExt(placeholder)),
				history(),
				drawSelection(),
				EditorView.lineWrapping,
				markdown(),
				syntaxHighlighting(highlight),
				highlightSelectionMatches(),
				keymap.of([...markdownKeymap, ...historyKeymap, ...searchKeymap, ...defaultKeymap]),
				theme,
				EditorView.contentAttributes.of({ 'aria-label': label }),
				EditorView.updateListener.of((u) => {
					if (u.docChanged) {
						const text = u.state.doc.toString();
						if (text !== value) value = text;
					}
					if (u.focusChanged) onfocus?.(u.view.hasFocus);
				})
			]
		});
		view = new EditorView({ state, parent });
		if (vim) armVim();
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
