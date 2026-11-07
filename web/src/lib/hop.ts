// hop.nvim's two-character jump for the composer's vim: `,` in normal or
// visual mode, two characters, and every match in view gets a label drawn
// over it while the rest of the text dims; typing a label puts the cursor
// there (in visual mode the selection extends through it). A panel under
// the text echoes what was typed, as Neovim's command line does. Labels
// come from the owner's keys (`asdfghjkl`), one character while they
// suffice, two beyond, never one a prefix of another; the nearest matches
// get the shortest. Escape or a key that is no label cancels. The editor's
// keydown handler calls `hopKey` while a hop is on and `startHop` on `,`.

import { StateEffect, StateField, type Extension } from '@codemirror/state';
import {
	Decoration,
	EditorView,
	WidgetType,
	showPanel,
	type DecorationSet,
	type Panel
} from '@codemirror/view';

export const DEFAULT_HOP_KEYS = 'asdfghjkl';

type Target = { from: number; to: number; label: string };
type Hop = {
	phase: 'chars' | 'labels';
	/** The characters typed so far: the pattern, then the label. */
	typed: string;
	/** The pattern, once typed. */
	pattern: string;
	targets: Target[];
	/** Extend the selection instead of moving the cursor (visual mode). */
	extend: boolean;
};

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
function decorate(hop: Hop): DecorationSet {
	if (hop.phase !== 'labels') return Decoration.none;
	const ranges = [];
	for (const t of hop.targets) {
		if (!t.label.startsWith(hop.typed)) continue;
		const rest = t.label.slice(hop.typed.length);
		const to = Math.min(t.from + rest.length, t.to);
		const widget = new Label(rest);
		ranges.push(
			to > t.from
				? Decoration.replace({ widget }).range(t.from, to)
				: Decoration.widget({ widget, side: 1 }).range(t.from)
		);
	}
	return Decoration.set(ranges, true);
}

/** The hop in progress, or null; set by the effect alone, dropped when the
 * text changes under it. */
const hopField = StateField.define<Hop | null>({
	create: () => null,
	update(hop, tr) {
		for (const e of tr.effects) if (e.is(setHop)) return e.value;
		return tr.docChanged ? null : hop;
	},
	provide: (f) => [
		EditorView.decorations.from(f, (hop) => (hop ? decorate(hop) : Decoration.none)),
		EditorView.editorAttributes.from(f, (hop) => ({
			class: hop?.phase === 'labels' ? 'cm-hopping' : ''
		})),
		showPanel.from(f, (hop) => (hop ? panel : null))
	]
});

/** What was typed, under the text: `,` then the characters, then the
 * label so far. */
function panel(view: EditorView): Panel {
	const dom = document.createElement('div');
	dom.className = 'cm-hop-panel';
	const render = () => {
		const hop = view.state.field(hopField);
		if (!hop) return;
		const typed = hop.phase === 'chars' ? hop.typed : hop.pattern + ' ' + hop.typed;
		dom.textContent = `,${typed}`;
		dom.title =
			hop.phase === 'chars'
				? 'Hop: two characters to look for'
				: `Hop: ${hop.targets.length} matches, type a label`;
	};
	render();
	return { dom, update: render };
}

const theme = EditorView.baseTheme({
	'.cm-hop-label': {
		display: 'inline-block',
		minWidth: '1ch',
		padding: '0 1px',
		borderRadius: '2px',
		backgroundColor: 'var(--primary)',
		color: 'var(--primary-foreground)',
		fontFamily: 'var(--font-mono)',
		fontWeight: '600',
		lineHeight: '1.2'
	},
	'&.cm-hopping .cm-content, &.cm-hopping .cm-gutters': {
		color: 'color-mix(in oklab, var(--foreground) 45%, transparent)'
	},
	'&.cm-hopping .cm-content .cm-hop-label': { color: 'var(--primary-foreground)' },
	'.cm-panels.cm-panels-bottom': {
		borderTop: '1px solid var(--border)',
		backgroundColor: 'transparent'
	},
	'.cm-hop-panel': {
		fontFamily: 'var(--font-mono)',
		fontSize: '0.75rem',
		color: 'var(--foreground)',
		padding: '1px 0.75rem',
		whiteSpace: 'pre'
	}
});

export function hopExtension(): Extension {
	return [hopField, theme];
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
	return found.slice(0, names.length).map((from, i) => ({
		from,
		to: view.state.doc.lineAt(from).to,
		label: names[i]
	}));
}

export function hopActive(view: EditorView): boolean {
	return view.state.field(hopField, false) !== null;
}

export function startHop(view: EditorView, extend: boolean) {
	view.dispatch({
		effects: setHop.of({ phase: 'chars', typed: '', pattern: '', targets: [], extend })
	});
}

export function cancelHop(view: EditorView) {
	if (view.state.field(hopField, false)) view.dispatch({ effects: setHop.of(null) });
}

/** A key while a hop is on; true when it was the hop's. */
export function hopKey(view: EditorView, event: KeyboardEvent, keys: string): boolean {
	const hop = view.state.field(hopField, false);
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
		const typed = hop.typed + event.key;
		if (typed.length < 2) {
			view.dispatch({ effects: setHop.of({ ...hop, typed }) });
			return true;
		}
		const targets = findTargets(view, typed, keys);
		if (!targets.length) {
			cancelHop(view);
			return true;
		}
		view.dispatch({
			effects: setHop.of({ ...hop, phase: 'labels', typed: '', pattern: typed, targets })
		});
		return true;
	}
	const next = hop.typed + event.key;
	const hit = hop.targets.find((t) => t.label === next);
	if (hit) {
		// NOTE: visual mode takes the match's first character in, as vim's inclusive motions do
		const head = hop.extend ? Math.min(hit.from + 1, hit.to) : hit.from;
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
	view.dispatch({ effects: setHop.of({ ...hop, typed: next }) });
	return true;
}
