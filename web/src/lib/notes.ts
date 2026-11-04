// What the panel derives from notes: labels, colours, links, slugs.

import type { Card, NoteKind, Priority, Status } from './api/types';
import { fmtDate } from './format';

const DAY_MS = 86_400_000;

export const KINDS: NoteKind[] = ['issue', 'memory', 'doc', 'report', 'progress'];

export const KIND_LABEL: Record<NoteKind, string> = {
	memory: 'Memory',
	issue: 'Issue',
	progress: 'Progress',
	doc: 'Doc',
	report: 'Report'
};

/** The kind's colour token (`layout.css`). */
export const KIND_COLOR: Record<NoteKind, string> = {
	memory: 'var(--color-kind-memory)',
	issue: 'var(--color-kind-issue)',
	progress: 'var(--color-kind-progress)',
	doc: 'var(--color-kind-doc)',
	report: 'var(--color-kind-report)'
};

/** Issues are open or closed; the rest keep the lifecycle names. */
export function statusLabel(n: { kind: NoteKind; status: Status }): string {
	if (n.kind === 'issue')
		return n.status === 'archived' ? 'closed' : n.status === 'active' ? 'open' : n.status;
	return n.status;
}

export const PRIORITY_RANK: Record<Priority, number> = { high: 0, normal: 1, low: 2 };

/** An issue's priority, kept in its `extra`. */
export function priorityOf(note: { extra: Record<string, unknown> }): Priority {
	const p = note.extra.priority;
	return p === 'high' || p === 'low' ? p : 'normal';
}

export const noteHref = (ws: string, slug: string) =>
	`/notes/${encodeURIComponent(ws)}/${encodeURIComponent(slug)}`;

/** The report viewer: the framed HTML with its note beside it. */
export const reportHref = (ws: string, slug: string) =>
	`/reports/${encodeURIComponent(ws)}/${encodeURIComponent(slug)}`;

/** Where a note opens: reports in their viewer, the rest as a page. */
export const openHref = (card: { kind: NoteKind; workspace: string; slug: string }) =>
	card.kind === 'report'
		? reportHref(card.workspace, card.slug)
		: noteHref(card.workspace, card.slug);

export const projectHref = (ws: string, project?: string | null) =>
	project
		? `/projects/${encodeURIComponent(ws)}/${encodeURIComponent(project)}`
		: `/projects/${encodeURIComponent(ws)}`;

/** A new-note editor for the workspace, kind and project. */
export function newNoteHref(ws: string, kind: NoteKind, project?: string | null): string {
	const q = new URLSearchParams({ ws, kind });
	if (project) q.set('project', project);
	return `/notes/new?${q}`;
}

/** Lowercase ASCII words joined with `-`, as the hub derives slugs. */
export const slugify = (text: string) =>
	text
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '')
		.slice(0, 80);

export const validSlug = (slug: string) => /^[a-z0-9][a-z0-9._-]*$/.test(slug) && slug.length <= 80;

/** The one line a list shows under the title. */
export const line = (c: Card) => c.summary || c.excerpt;

/** What a session started for an issue is told; the sheet shows it to edit. */
export const issuePrompt = (slug: string, title: string) =>
	`Work on the issue [[${slug}]] ("${title}"). Read it first with \`recall\`, and the notes it links to. It is already claimed for this session; if you claim it again, pass \`issue\` so the board keeps it. If it needs a decision from me, ask before implementing. Log progress on the issue as you go, and close it with \`issue_update\` once the work is done and verified.`;

/** Day key for grouping, local time. */
export const dayKey = (ts: number) => new Date(ts).toDateString();

/** A day's heading, seen from `now`. */
export function dayName(ts: number, now: number): string {
	const today = dayKey(now);
	if (dayKey(ts) === today) return 'Today';
	if (dayKey(ts + DAY_MS) === today) return 'Yesterday';
	return fmtDate(ts);
}

/** A progress entry's excerpt without the title it starts with. */
export function entryRest(e: { title: string; excerpt: string }): string {
	const head = e.title.replace(/…$/, '');
	const text = e.excerpt.replace(/^[-*\s]+/, '');
	if (!text.startsWith(head)) return text;
	const rest = text.slice(head.length).trim();
	return rest && e.title.endsWith('…') ? `…${rest}` : rest;
}
