// The composer's drafts: kept on the hub (`live.drafts`, `putDraft`) so a
// prompt begun on the phone is on the desktop, and in localStorage as the
// copy for a reload before the hub answers. A field writes through
// `noteDraft` on every change (the hub after a short pause), and adopts
// what the hub says through `remoteDraft` when it did not type meanwhile.

import { Unauthorized } from './api/client';
import { putDraft, type Draft } from './api/index';
import { live } from './live/store.svelte';
import { prefs } from './prefs.svelte';

/** How long after the last keystroke the hub gets the draft. */
export const DRAFT_DEBOUNCE_MS = 800;

/** The draft key of a session's composer. */
export const sessionDraft = (id: string) => `session:${id}`;

/** The machine "Edit in nvim" opens the editor on: the one Settings names,
 * else `preferred` (the session's host, the start sheet's), else the first
 * connected one whose collector opens an editor; null when none can. */
export function editHostFor(preferred?: string | null): string | null {
	const can = (name: string | null | undefined) =>
		Boolean(name) && live.hosts.some((h) => h.name === name && h.connected && h.edit);
	if (can(prefs.editHost)) return prefs.editHost;
	if (can(preferred)) return preferred ?? null;
	return live.hosts.find((h) => h.connected && h.edit)?.name ?? null;
}

/** What the hub has of each key, as far as this panel knows: what it sent
 * and what it heard. A stream update with another text came from elsewhere. */
const synced: Record<string, string> = {};
const timers: Record<string, ReturnType<typeof setTimeout>> = {};
const pending: Record<string, string> = {};

/** The text a field starts with: the hub's, else this device's copy. */
export function initialDraft(key: string): string {
	const hub = live.drafts.get(key);
	if (hub) {
		synced[key] = hub.text;
		return hub.text;
	}
	return prefs.draft(key);
}

/** The field's text changed: kept here at once, sent to the hub after a
 * pause (an empty text drops the draft). */
export function noteDraft(key: string, text: string) {
	prefs.setDraft(key, text);
	if (text === synced[key] || (text === '' && synced[key] === undefined)) {
		clearTimeout(timers[key]);
		delete pending[key];
		return;
	}
	pending[key] = text;
	clearTimeout(timers[key]);
	timers[key] = setTimeout(() => flushDraft(key), DRAFT_DEBOUNCE_MS);
}

/** Sends what waits for the hub now (before a send, an edit, leaving). */
export async function flushDraft(key: string): Promise<void> {
	clearTimeout(timers[key]);
	const text = pending[key];
	if (text === undefined) return;
	delete pending[key];
	if (live.drafts.get(key)?.editing) return;
	try {
		const { draft } = await putDraft(key, text);
		synced[key] = draft.text;
		live.putDraft(draft);
	} catch (e) {
		if (e instanceof Unauthorized) live.needToken = true;
		// NOTE: offline: the local copy stands; the next change tries again
		pending[key] = pending[key] ?? text;
	}
}

/** A draft was sent as a prompt: nothing is left of it anywhere. */
export function clearDraft(key: string) {
	clearTimeout(timers[key]);
	delete pending[key];
	prefs.setDraft(key, '');
	if (synced[key]) {
		synced[key] = '';
		putDraft(key, '')
			.then(({ draft }) => live.putDraft(draft))
			.catch(() => {});
	}
}

/** The hub's draft changed: the text to take, or null when it is this
 * panel's own write coming back, or the field typed since (its text wins
 * and goes up next). An editor's result lands: the field waited on it. */
export function remoteDraft(key: string, draft: Draft | undefined): string | null {
	if (!draft || draft.editing) return null;
	if (draft.text === synced[key]) return null;
	if (pending[key] !== undefined) return null;
	synced[key] = draft.text;
	return draft.text;
}

/** What `remoteDraft` compares against, after a send through another path
 * (the edit request carries the text itself). */
export function markSynced(key: string, text: string) {
	synced[key] = text;
	clearTimeout(timers[key]);
	delete pending[key];
}
