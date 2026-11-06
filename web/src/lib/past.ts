// Past sessions (Sessions › Past): the filters as they live in the URL,
// the choices the bar offers, and the small formatting the cards share.

import type { PastSession, PastSort } from './api/types';
import { fmtTime } from './format';

/** The page's address. */
export const PAST = '/sessions/past';

export type Filters = {
	q: string;
	hosts: string[];
	/** A facet key (`ProjectCount.key`), or '' for every project. */
	project: string;
	/** A `WHEN` id, or '' for any time. */
	when: string;
	/** '' is the default: best match with a search, else last active. */
	sort: PastSort | '';
};

const DAY_MS = 86_400_000;

export const WHEN: { id: string; label: string; ms: number }[] = [
	{ id: '1d', label: 'Last 24 hours', ms: DAY_MS },
	{ id: '7d', label: 'Last 7 days', ms: 7 * DAY_MS },
	{ id: '30d', label: 'Last 30 days', ms: 30 * DAY_MS },
	{ id: '90d', label: 'Last 90 days', ms: 90 * DAY_MS },
	{ id: '365d', label: 'Last year', ms: 365 * DAY_MS }
];

export const SORTS: { id: PastSort; label: string; needsQuery?: boolean }[] = [
	{ id: 'best', label: 'Best match', needsQuery: true },
	{ id: 'recent', label: 'Last active' },
	{ id: 'started', label: 'Started' },
	{ id: 'prompts', label: 'Most prompts' },
	{ id: 'size', label: 'Largest' },
	{ id: 'cost', label: 'Most expensive' }
];

const SORT_IDS = new Set<string>(SORTS.map((s) => s.id));

export function readFilters(params: Pick<URLSearchParams, 'get'>): Filters {
	const sort = params.get('sort') ?? '';
	return {
		q: params.get('q') ?? '',
		hosts: (params.get('host') ?? '').split(',').filter(Boolean),
		project: params.get('project') ?? '',
		when: WHEN.some((w) => w.id === params.get('when')) ? params.get('when')! : '',
		sort: SORT_IDS.has(sort) ? (sort as PastSort) : ''
	};
}

/** The page's URL for `filters` and the open session. */
export function pastHref(filters: Filters, id: string | null = null): string {
	const p = new URLSearchParams();
	if (filters.q.trim()) p.set('q', filters.q.trim());
	if (filters.hosts.length) p.set('host', filters.hosts.join(','));
	if (filters.project) p.set('project', filters.project);
	if (filters.when) p.set('when', filters.when);
	if (filters.sort) p.set('sort', filters.sort);
	if (id) p.set('id', id);
	const s = p.toString();
	return s ? `${PAST}?${s}` : PAST;
}

/** The order the list is in. */
export const sortOf = (f: Filters): PastSort => f.sort || (f.q.trim() ? 'best' : 'recent');

export const sinceOf = (when: string, now: number): number | undefined => {
	const w = WHEN.find((x) => x.id === when);
	return w ? Math.round(now - w.ms) : undefined;
};

/** A facet key in words: `ws › project`, `ws (the workspace itself)`. */
export function projectLabel(key: string): string {
	if (key === '-') return 'Outside the workspaces';
	const [ws, project] = key.split('/');
	return project ? `${ws} › ${project}` : `${ws} (the workspace itself)`;
}

/** A snippet's text in pieces, odd ones matched (`\u0002…\u0003`). */
const MARK_OPEN = String.fromCharCode(2);
const MARK_CLOSE = String.fromCharCode(3);

export const marks = (snippet: string) =>
	snippet.replaceAll(MARK_CLOSE, MARK_OPEN).split(MARK_OPEN);

/** The session's name for lists: its title, else what you asked first. */
export const pastTitle = (s: PastSession) =>
	s.title || s.first_prompt?.split('\n')[0] || `Session ${s.id.slice(0, 8)}`;

/** Claude Code with no terminal (`claude -p`, the SDK). */
export const headless = (s: PastSession) =>
	Boolean(s.entrypoint && s.entrypoint !== 'cli' && s.entrypoint !== 'claude-desktop');

/** Why it cannot be resumed from the panel, or null when it can. */
export function cannotResume(s: PastSession): string | null {
	if (s.open) return 'It is running now: open it instead.';
	if (!s.workspace) return 'It ran outside every workspace: resume it in a terminal.';
	return null;
}

/** What to type in a terminal on a machine that syncs. */
export const resumeCommand = (s: PastSession) =>
	`cd ${shellQuote(s.cwd)} && claude --resume ${s.id}`;

function shellQuote(path: string): string {
	return /^[\w@%+=:,./-]+$/.test(path) ? path : `'${path.replace(/'/g, `'\\''`)}'`;
}

/** `claude-opus-5-5` → `Opus 5.5`. */
export function modelName(model: string | null): string | null {
	if (!model) return null;
	const m = model.match(/^claude-([a-z]+)-(\d+)(?:-(\d+))?/);
	if (!m) return model;
	const family = m[1][0].toUpperCase() + m[1].slice(1);
	return m[3] ? `${family} ${m[2]}.${m[3]}` : `${family} ${m[2]}`;
}

const weekday = new Intl.DateTimeFormat(undefined, { weekday: 'short' });
const dayMonth = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short' });
const dayMonthYear = new Intl.DateTimeFormat(undefined, {
	day: 'numeric',
	month: 'short',
	year: 'numeric'
});
const full = new Intl.DateTimeFormat(undefined, {
	weekday: 'short',
	day: 'numeric',
	month: 'short',
	year: 'numeric',
	hour: '2-digit',
	minute: '2-digit'
});

/** When, as close as it matters: `14:05`, `yesterday 14:05`, `Mon 14:05`,
 * `3 Nov`, `3 Nov 2025`. */
export function fmtWhenShort(ts: number, now: number): string {
	const d = new Date(ts);
	const today = new Date(now);
	if (d.toDateString() === today.toDateString()) return fmtTime(ts);
	if (d.toDateString() === new Date(now - DAY_MS).toDateString()) return `yesterday ${fmtTime(ts)}`;
	if (now - ts < 6 * DAY_MS) return `${weekday.format(d)} ${fmtTime(ts)}`;
	return d.getFullYear() === today.getFullYear() ? dayMonth.format(d) : dayMonthYear.format(d);
}

/** The exact time, for tooltips. */
export const fmtFull = (ts: number) => full.format(new Date(ts));

/** `3 min ago`, `5 h ago`, `4 days ago`, `7 weeks ago`, `5 months ago`. */
export function fmtAgo(ts: number, now: number): string {
	const min = Math.max(0, Math.round((now - ts) / 60_000));
	if (min < 1) return 'just now';
	if (min < 60) return `${min} min ago`;
	const h = Math.round(min / 60);
	if (h < 24) return `${h} h ago`;
	const days = Math.round(h / 24);
	if (days < 14) return `${days} day${days === 1 ? '' : 's'} ago`;
	if (days < 60) return `${Math.round(days / 7)} weeks ago`;
	const months = Math.round(days / 30);
	return months < 12 ? `${months} months ago` : `${Math.round(days / 365)} y ago`;
}

const HIGHLIGHT = 'past-match';

/** Marks the words of `q` (as word prefixes, like the search) in `root`'s
 * text with the CSS highlight `past-match`; returns the cleanup. Leaves the
 * DOM alone (rendered markdown, code blocks) and does nothing where the
 * browser lacks the Custom Highlight API. */
export function highlightTerms(root: HTMLElement, q: string): () => void {
	const words = q
		.split(/[^\p{L}\p{N}_]+/u)
		.filter((w) => w.length > 1)
		.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
	if (!words.length || typeof CSS === 'undefined' || !('highlights' in CSS)) return () => {};
	const pattern = new RegExp(`(?<![\\p{L}\\p{N}_])(?:${words.join('|')})`, 'giu');
	const ranges: Range[] = [];
	const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
	for (let node = walker.nextNode(); node; node = walker.nextNode()) {
		const text = node.textContent ?? '';
		for (const m of text.matchAll(pattern)) {
			const range = new Range();
			range.setStart(node, m.index);
			range.setEnd(node, m.index + m[0].length);
			ranges.push(range);
		}
	}
	const existing = CSS.highlights.get(HIGHLIGHT);
	const highlight = existing ?? new Highlight();
	for (const r of ranges) highlight.add(r);
	if (!existing) CSS.highlights.set(HIGHLIGHT, highlight);
	return () => {
		for (const r of ranges) highlight.delete(r);
	};
}
