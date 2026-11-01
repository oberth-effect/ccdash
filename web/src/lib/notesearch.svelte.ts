// The search over every note. The Projects page and the command palette are
// both this: full text on the hub (titles, summaries and bodies, by word
// prefix), a kind (the buttons, or `kind:issue` in the text), the retired
// notes, and an order.

import { untrack } from 'svelte';
import { Unauthorized } from './api/client';
import { fetchNotes } from './api/index';
import type { Hit, NoteKind } from './api/types';
import { live } from './live/store.svelte';

export type Sort = 'relevance' | 'updated' | 'recalls';

export const SORTS: { id: Sort; label: string }[] = [
	{ id: 'relevance', label: 'Best match' },
	{ id: 'updated', label: 'Newest' },
	{ id: 'recalls', label: 'Most recalled' }
];

const WAIT_MS = 250;
const LIMIT = 60;
const KIND_PREFIX = /^kind:(\w+)\s*/;

export type SearchInput = {
	text: string;
	/** Workspaces as the API takes them; none is all of them. */
	workspace?: string;
	/** False keeps it quiet (a closed palette). */
	enabled?: boolean;
	/** Hits at most. */
	limit?: number;
};

export class NoteSearch {
	kind = $state<NoteKind | ''>('');
	sort = $state<Sort>('relevance');
	retired = $state(true);
	/** The last answer; null while there is nothing to search for. */
	hits = $state<Hit[] | null>(null);
	searching = $state(false);
	/** The hits in the picked order (the hub's is by match). */
	shown = $derived.by(() => {
		const hits = this.hits;
		if (!hits || this.sort === 'relevance') return hits;
		const by = this.sort === 'updated' ? 'updated_at' : 'recalls';
		return [...hits].sort((a, b) => b[by] - a[by]);
	});

	#asked = 0;

	/** Create it while a component initialises: it watches `read` for as
	 * long as that component lives. */
	constructor(read: () => SearchInput) {
		$effect(() => {
			const { text, workspace, enabled = true, limit = LIMIT } = read();
			const q = text.trim();
			const [kind, retired] = [this.kind, this.retired];
			const asked = ++this.#asked;
			if (!enabled || (!q && !kind)) {
				this.hits = null;
				this.searching = false;
				return;
			}
			const timer = setTimeout(
				() => untrack(() => this.#search(asked, q, workspace, kind, retired, limit)),
				WAIT_MS
			);
			return () => clearTimeout(timer);
		});
	}

	async #search(
		asked: number,
		q: string,
		workspace: string | undefined,
		kind: NoteKind | '',
		retired: boolean,
		limit: number
	) {
		this.searching = true;
		const prefix = q.match(KIND_PREFIX);
		try {
			const { hits } = await fetchNotes({
				q: (prefix ? q.slice(prefix[0].length) : q) || undefined,
				kind: prefix?.[1] ?? (kind || undefined),
				workspace,
				status: retired ? 'active,stale,archived,superseded' : 'active,stale',
				limit
			});
			// NOTE: a slow answer must not replace the one to a newer question
			if (asked === this.#asked) this.hits = hits;
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			if (asked === this.#asked) this.hits = [];
		} finally {
			if (asked === this.#asked) this.searching = false;
		}
	}
}
