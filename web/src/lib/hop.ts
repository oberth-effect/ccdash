// hop.nvim's two-character jump for the composer's vim: `,` in normal or
// visual mode, two characters, and every match in view gets a label drawn
// over it; typing a label puts the cursor there (in visual mode the
// selection extends to it). Labels come from the owner's keys
// (`asdfghjkl`), one character while they suffice, two beyond, never one
// a prefix of another; the nearest matches get the shortest. Escape or a
// key that is no label cancels. The whole thing is the editor's keydown
// handler calling `hopKey` while a hop is on, and `startHop` on `,`.

import { StateEffect, type Extension } from '@codemirror/state';
import { Decoration, EditorView, WidgetType, type DecorationSet } from '@codemirror/view';

export const DEFAULT_HOP_KEYS = 'asdfghjkl';

type Target = { from: number; label: string };
type Hop = {
	phase: 'chars' | 'labels';
	/** The characters typed so far: the pattern, then the label. */
	typed: string;
	targets: Target[];
	/** Extend the selection instead of moving the cursor (visual mode). */
	extend: boolean;
};

const hops = new WeakMap<EditorView, Hop>();
const setHop = StateEffect.define<Hop | null>();

class Label extends WidgetType {
	text: string;
	constructor(text: string) {
		super();
		this.text = text;
	}
	eq(other: Label) {
		return other.text === this.text;
	}
	toDOM() {
		const span = document.createElement('span');
		span.className = 'cm-hop-label';
		span.textContent = this.text;
		return span;
	}
	ignoreEvent() {
		return true;
	}
}

/** The labels drawn over the matches still possible. */
function decorate(hop: Hop, view: EditorView): DecorationSet {
	if (hop.phase !== 'labels') return Decoration.none;
	const ranges = [];
	for (const t of hop.targets) {
		if (!t.label.startsWith(hop.typed)) continue;
		const rest = t.label.slice(hop.typed.length);
		const line = view.state.doc.lineAt(t.from);
		const to = Math.min(t.from + rest.length, line.to);
		const widget = new Label(rest);
		ranges.push(
			to > t.from
				? Decoration.replace({ widget }).range(t.from, to)
				: Decoration.widget({ widget, side: 1 }).range(t.from)
		);
	}
	return Decoration.set(ranges, true);
}

const theme = EditorView.baseTheme({
	'.cm-hop-label': {
		display: 'inline-block',
		minWidth: '1ch',
		padding: '0 1px',
		borderRadius: '2px',
		backgroundColor: 'var(--permission)',
		color: '#fff',
		fontFamily: 'var(--font-mono)',
		fontWeight: '600',
		lineHeight: '1.2'
	}
});

/** The labels, read from the view's hop at every update: a dispatch with
 * `setHop` (nothing else changes) is enough to redraw. NOTE: a function
 * source may not add block or line-breaking decorations; these labels sit
 * inside one line each. */
const hopDecorations = EditorView.decorations.of((view) => {
	const hop = hops.get(view);
	return hop ? decorate(hop, view) : Decoration.none;
});

export function hopExtension(): Extension {
	return [theme, hopDecorations];
}

/** Labels for `n` targets from `keys`, none a prefix of another: single
 * keys while they suffice, else the last keys become the first character
 * of two-character labels. Targets past what the keys cover get none. */
export function hopLabels(n: number, keys: string): string[] {
	const k = keys.length;
	if (k < 2) return [];
	if (n <= k) return [...keys].slice(0, n);
	const groups = Math.min(k, Math.ceil((n - k) / (k - 1)));
	const singles = k - groups;
	const out: string[] = [...keys].slice(0, singles);
	for (const lead of [...keys].slice(singles)) for (const key of keys) out.push(lead + key);
	return out.slice(0, n);
}

/** Every start of `query` in the visible text, nearest to the cursor first. */
function findTargets(view: EditorView, query: string, keys: string): Target[] {
	const q = query.toLowerCase();
	const cursor = view.state.selection.main.head;
	const found: number[] = [];
	for (const { from, to } of view.visibleRanges) {
		const text = view.state.sliceDoc(from, to).toLowerCase();
		let at = text.indexOf(q);
		while (at !== -1) {
			found.push(from + at);
			at = text.indexOf(q, at + 1);
		}
	}
	found.sort((a, b) => Math.abs(a - cursor) - Math.abs(b - cursor) || a - b);
	const names = hopLabels(found.length, keys);
	return found.slice(0, names.length).map((from, i) => ({ from, label: names[i] }));
}

export function hopActive(view: EditorView): boolean {
	return hops.has(view);
}

export function startHop(view: EditorView, extend: boolean) {
	hops.set(view, { phase: 'chars', typed: '', targets: [], extend });
}

function redraw(view: EditorView) {
	view.dispatch({ effects: setHop.of(hops.get(view) ?? null) });
}

export function cancelHop(view: EditorView) {
	if (!hops.delete(view)) return;
	redraw(view);
}

/** A key while a hop is on; true when it was the hop's. */
export function hopKey(view: EditorView, event: KeyboardEvent, keys: string): boolean {
	const hop = hops.get(view);
	if (!hop) return false;
	if (event.ctrlKey || event.metaKey || event.altKey) {
		cancelHop(view);
		return false;
	}
	if (event.key === 'Escape') {
		cancelHop(view);
		return true;
	}
	if (event.key.length !== 1) return event.key !== 'Shift';
	if (hop.phase === 'chars') {
		hop.typed += event.key;
		if (hop.typed.length < 2) return true;
		hop.targets = findTargets(view, hop.typed, keys);
		hop.typed = '';
		if (!hop.targets.length) {
			cancelHop(view);
			return true;
		}
		hop.phase = 'labels';
		redraw(view);
		return true;
	}
	const next = hop.typed + event.key;
	const hit = hop.targets.find((t) => t.label === next);
	if (hit) {
		hops.delete(view);
		// NOTE: visual mode takes the match's first character in, as vim's inclusive motions do
		const line = view.state.doc.lineAt(hit.from);
		const head = hop.extend ? Math.min(hit.from + 1, line.to) : hit.from;
		const anchor = hop.extend ? view.state.selection.main.anchor : hit.from;
		view.dispatch({
			selection: { anchor, head },
			effects: setHop.of(null),
			scrollIntoView: true
		});
		return true;
	}
	if (!hop.targets.some((t) => t.label.startsWith(next))) {
		cancelHop(view);
		return true;
	}
	hop.typed = next;
	redraw(view);
	return true;
}
