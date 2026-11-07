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
/** `p` reads the clipboard (else the register, which in-page yanks and the
 * browser's own paste keep in step). */
let pasteReads = true;
/** A clipboard read is on its way; a second `p` meanwhile is dropped. */
let reading = false;

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
export function setClipboard(on: boolean, readOnPaste = true) {
	clipboardOn = on;
	pasteReads = readOnPaste;
	// NOTE: the helper's hello goes out now, so it has answered before the first `p`
	if (on) listenToBridge();
	if (!on || clipboardArmed) return;
	clipboardArmed = true;
	const rc = Vim.getRegisterController() as unknown as RegisterController;
	const push = rc.pushText;
	rc.pushText = function (name, operator, text, linewise, blockwise) {
		push.call(this, name, operator, text, linewise, blockwise);
		if (!clipboardOn || (name && name !== '"') || !text) return;
		// NOTE: a linewise yank (`Vy`) gets its newline from the engine after this
		// point; the clipboard must end with it too, or `p` pastes it inline
		const out = linewise && !text.endsWith('\n') ? text + '\n' : text;
		navigator.clipboard?.writeText(out).catch(() => {});
	};
}

// ---- the Firefox helper: an extension that reads the clipboard for the page

const BRIDGE_HERE = 'ccdash:clipboard-bridge';
const BRIDGE_HELLO = 'ccdash:clipboard-hello';
const BRIDGE_ASK = 'ccdash:clipboard-read';
const BRIDGE_ANSWER = 'ccdash:clipboard-text';
/** The helper answers within this, or the page reads the clipboard itself. */
const BRIDGE_WAIT_MS = 1000;

let bridgePresent = false;
let bridgeListening = false;
const bridgeWatchers = new Set<(present: boolean) => void>();
let bridgeNext = 1;
const bridgeWaiting = new Map<number, (text: string | null) => void>();

/** Whether the Firefox helper extension (`firefox-ext/`) is on this page:
 * it reads the clipboard with no prompt. */
export function clipboardBridge(): boolean {
	listenToBridge();
	return bridgePresent;
}

/** Tells `cb` now and whenever the helper announces itself (it answers
 * the page's hello, so a page loaded after it still learns); the
 * returned function stops that. Settings shows it. */
export function bridgeWatch(cb: (present: boolean) => void): () => void {
	listenToBridge();
	bridgeWatchers.add(cb);
	cb(bridgePresent);
	return () => bridgeWatchers.delete(cb);
}

function listenToBridge() {
	if (bridgeListening || typeof window === 'undefined') return;
	bridgeListening = true;
	window.addEventListener('message', (event) => {
		if (event.source !== window || event.origin !== location.origin) return;
		const msg = event.data as { type?: string; id?: number; text?: string | null } | null;
		if (!msg) return;
		if (msg.type === BRIDGE_HERE) {
			bridgePresent = true;
			for (const cb of bridgeWatchers) cb(true);
		} else if (msg.type === BRIDGE_ANSWER && typeof msg.id === 'number') {
			bridgeWaiting.get(msg.id)?.(typeof msg.text === 'string' ? msg.text : null);
			bridgeWaiting.delete(msg.id);
		}
	});
	window.postMessage({ type: BRIDGE_HELLO }, location.origin);
}

/** The clipboard through the helper, or null when it is not there or
 * does not answer in time. */
function readThroughBridge(): Promise<string | null> {
	listenToBridge();
	if (!bridgePresent) return Promise.resolve(null);
	return new Promise((resolve) => {
		const id = bridgeNext++;
		const timer = setTimeout(() => {
			bridgeWaiting.delete(id);
			resolve(null);
		}, BRIDGE_WAIT_MS);
		bridgeWaiting.set(id, (text) => {
			clearTimeout(timer);
			resolve(text);
		});
		window.postMessage({ type: BRIDGE_ASK, id }, location.origin);
	});
}

/** The clipboard's text: the helper first (no prompt), else the browser's
 * own read (Chromium asks once, Firefox shows its Paste button); null
 * when neither gives it. */
async function readClipboard(): Promise<string | null> {
	const viaBridge = await readThroughBridge();
	if (viaBridge !== null) return viaBridge;
	if (!navigator.clipboard?.readText) return null;
	try {
		return await navigator.clipboard.readText();
	} catch {
		return null;
	}
}

/** `p` or `P` in normal or visual mode with no register named: the
 * clipboard's text becomes the unnamed register, then the key runs as
 * usual (counts stay pending meanwhile). True when taken. A read the
 * browser refuses falls back to the register as it is. */
export function pasteFromClipboard(view: EditorView, event: KeyboardEvent): boolean {
	if (!clipboardOn || !pasteReads || event.ctrlKey || event.metaKey || event.altKey) return false;
	if (event.key !== 'p' && event.key !== 'P') return false;
	if (reading) return true;
	const cm = getCM(view) as Parameters<typeof Vim.handleEx>[0] | null;
	const state = cm?.state.vim as
		{ insertMode: boolean; inputState: { registerName?: string; keyBuffer: string[] } } | undefined;
	if (!cm || !state || state.insertMode) return false;
	if (state.inputState.registerName || state.inputState.keyBuffer.length) return false;
	if (!clipboardBridge() && !navigator.clipboard?.readText) return false;
	const key = event.key;
	const rc = Vim.getRegisterController() as unknown as RegisterController;
	reading = true;
	readClipboard()
		.then((text) => {
			if (text) rc.getRegister('"').setText(text, text.endsWith('\n'));
		})
		.finally(() => {
			reading = false;
			Vim.handleKey(cm, key, 'user');
		});
	return true;
}

/** The browser's own paste (Ctrl+Shift+V, Shift+Insert, the menu) in normal
 * or visual mode: its text becomes the unnamed register and `p` runs, so
 * it lands as a paste would, with no clipboard prompt anywhere (the
 * browser hands the text over itself). True when taken; insert mode keeps
 * the native paste. */
export function pasteEvent(view: EditorView, event: ClipboardEvent): boolean {
	if (!clipboardOn) return false;
	const cm = getCM(view) as Parameters<typeof Vim.handleEx>[0] | null;
	const state = cm?.state.vim as { insertMode: boolean } | undefined;
	if (!cm || !state || state.insertMode) return false;
	const text = event.clipboardData?.getData('text/plain') ?? '';
	if (!text) return false;
	const rc = Vim.getRegisterController() as unknown as RegisterController;
	rc.getRegister('"').setText(text, text.endsWith('\n'));
	Vim.handleKey(cm, 'p', 'user');
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
