// The composer's vim keys (`@replit/codemirror-vim`): the ex commands that
// talk to the panel (`:w` saves the draft, `:x`, `:wq` and `:q` are done
// editing) and the owner's vimrc (Settings), applied to every editor.
// `Vim` is one object for the whole page, so the commands are defined once
// and find the editor's handlers through `hooks`.

import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { Vim, getCM, vim } from '@replit/codemirror-vim';

export type VimHooks = {
	/** `:w` */
	onwrite?: () => void;
	/** `:x`, `:wq`, `:q` */
	onquit?: () => void;
};

const hooks = new WeakMap<EditorView, VimHooks>();
let defined = false;

export function setVimHooks(view: EditorView, h: VimHooks) {
	hooks.set(view, h);
}

/** `cm` is the CM5-style adapter the vim engine hands to ex commands. */
function viewOf(cm: unknown): EditorView | null {
	const v = (cm as { cm6?: EditorView } | null)?.cm6;
	return v instanceof EditorView ? v : null;
}

function defineEx() {
	if (defined) return;
	defined = true;
	const write = (cm: unknown) => {
		const v = viewOf(cm);
		if (v) hooks.get(v)?.onwrite?.();
	};
	const quit = (cm: unknown) => {
		const v = viewOf(cm);
		if (v) hooks.get(v)?.onquit?.();
	};
	Vim.defineEx('write', 'w', write);
	Vim.defineEx('quit', 'q', quit);
	Vim.defineEx('wq', undefined, (cm: unknown) => {
		write(cm);
		quit(cm);
	});
	Vim.defineEx('xit', 'x', (cm: unknown) => {
		write(cm);
		quit(cm);
	});
}

/** The vim extension, with the panel's ex commands in place. */
export function vimExtension() {
	defineEx();
	return vim();
}

// ---- the system clipboard as the unnamed register (`clipboard=unnamedplus`)

let clipboardOn = false;
let clipboardArmed = false;

type RegisterController = {
	pushText: (
		name: string | null | undefined,
		operator: string,
		text: string,
		linewise?: boolean,
		blockwise?: boolean
	) => void;
	getRegister: (name?: string) => { setText: (text: string, linewise?: boolean) => void };
};

/** Whether yanks and deletes go to the system clipboard and `p` reads it.
 * The engine's own `+` register already does both; this routes the
 * unnamed register there: every push to it is written out, and a `p`
 * without a register reads the clipboard first (`pasteFromClipboard`). */
export function setClipboard(on: boolean) {
	clipboardOn = on;
	if (!on || clipboardArmed) return;
	clipboardArmed = true;
	const rc = Vim.getRegisterController() as unknown as RegisterController;
	const push = rc.pushText;
	rc.pushText = function (name, operator, text, linewise, blockwise) {
		push.call(this, name, operator, text, linewise, blockwise);
		if (clipboardOn && (!name || name === '"') && text)
			navigator.clipboard?.writeText(text).catch(() => {});
	};
}

/** `p` or `P` in normal or visual mode with no register named: the
 * clipboard's text becomes the unnamed register, then the key runs as
 * usual (counts stay pending meanwhile). True when taken. A read the
 * browser refuses (Firefox asks with a small Paste button the first time
 * the text came from elsewhere) falls back to the register as it is. */
export function pasteFromClipboard(view: EditorView, event: KeyboardEvent): boolean {
	if (!clipboardOn || event.ctrlKey || event.metaKey || event.altKey) return false;
	if (event.key !== 'p' && event.key !== 'P') return false;
	const cm = getCM(view) as Parameters<typeof Vim.handleEx>[0] | null;
	const state = cm?.state.vim as
		{ insertMode: boolean; inputState: { registerName?: string; keyBuffer: string[] } } | undefined;
	if (!cm || !state || state.insertMode) return false;
	if (state.inputState.registerName || state.inputState.keyBuffer.length) return false;
	if (!navigator.clipboard?.readText) return false;
	const key = event.key;
	const rc = Vim.getRegisterController() as unknown as RegisterController;
	navigator.clipboard
		.readText()
		.then((text) => {
			if (text) rc.getRegister('"').setText(text, text.endsWith('\n'));
		})
		.catch(() => {})
		.finally(() => {
			Vim.handleKey(cm, key, 'user');
		});
	return true;
}

/** Runs the vimrc's lines on `view`'s vim; the lines it did not take, with
 * why. Comments (`"`) and blank lines are skipped. */
export function applyVimrc(view: EditorView, text: string): string[] {
	// NOTE: `getCM` types the adapter before vim's state exists on it; it does by now
	const cm = getCM(view) as Parameters<typeof Vim.handleEx>[0] | null;
	if (!cm) return ['the editor has no vim'];
	const errors: string[] = [];
	// NOTE: vim reports a bad ex command as a notification, not an error:
	// the notification is caught here while the lines run
	const notifier = cm as unknown as {
		openNotification: (node: Node, options: unknown) => () => void;
	};
	const shown = notifier.openNotification;
	const said: string[] = [];
	notifier.openNotification = (node) => {
		said.push(node.textContent?.trim() ?? '');
		return () => {};
	};
	try {
		for (const [i, raw] of text.split('\n').entries()) {
			const line = raw.trim();
			if (!line || line.startsWith('"')) continue;
			try {
				Vim.handleEx(cm, line.replace(/^:/, ''));
			} catch (e) {
				said.push((e as Error).message || String(e));
			}
			const why = said.splice(0).filter(Boolean).join('; ');
			if (why) errors.push(`line ${i + 1}: ${why.replace(/\s+Press ENTER.*$/, '')}`);
		}
	} finally {
		notifier.openNotification = shown;
	}
	return errors;
}

/** Tries the vimrc on a throwaway editor (Settings, before saving). */
export function checkVimrc(text: string): string[] {
	const host = document.createElement('div');
	const view = new EditorView({
		state: EditorState.create({ doc: '', extensions: [vimExtension()] }),
		parent: host
	});
	try {
		return applyVimrc(view, text);
	} finally {
		view.destroy();
	}
}

/** The mode's short name for the toolbar. */
export function modeLabel(mode: string, subMode?: string): string {
	if (mode === 'visual')
		return subMode === 'linewise' ? 'V-LINE' : subMode === 'blockwise' ? 'V-BLOCK' : 'VISUAL';
	return mode.toUpperCase();
}
