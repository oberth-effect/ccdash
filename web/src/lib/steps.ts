// What the chat makes of a run of tool calls: a row per step with an icon
// and a sentence (what Claude said the call does, else what it touched),
// same-tool file operations folded into one row, and the run's one-line
// summary. Older events without a description fall back to the command.

import type { Component } from 'svelte';
import BotIcon from '@lucide/svelte/icons/bot';
import FilePenIcon from '@lucide/svelte/icons/file-pen';
import FileTextIcon from '@lucide/svelte/icons/file-text';
import GlobeIcon from '@lucide/svelte/icons/globe';
import ListChecksIcon from '@lucide/svelte/icons/list-checks';
import MessageSquareIcon from '@lucide/svelte/icons/message-square';
import PlugIcon from '@lucide/svelte/icons/plug';
import SearchIcon from '@lucide/svelte/icons/search';
import TerminalIcon from '@lucide/svelte/icons/terminal';
import WrenchIcon from '@lucide/svelte/icons/wrench';
import type { Item, Step } from './api/types';
import { fmtDuration, plural, toolName } from './format';

export type Tool = Extract<Item, { kind: 'tool' }>;

export type Family =
	'shell' | 'read' | 'edit' | 'search' | 'web' | 'agent' | 'plugin' | 'ask' | 'task' | 'other';

const FAMILIES: Record<string, Family> = {
	Bash: 'shell',
	Read: 'read',
	Edit: 'edit',
	Write: 'edit',
	MultiEdit: 'edit',
	NotebookEdit: 'edit',
	Grep: 'search',
	Glob: 'search',
	ToolSearch: 'search',
	WebFetch: 'web',
	WebSearch: 'web',
	Agent: 'agent',
	Task: 'agent',
	SendMessage: 'agent',
	TaskStop: 'agent',
	Monitor: 'agent',
	Workflow: 'agent',
	AskUserQuestion: 'ask',
	ExitPlanMode: 'ask',
	EnterPlanMode: 'ask',
	TaskCreate: 'task',
	TaskUpdate: 'task',
	TaskList: 'task',
	TaskGet: 'task',
	TodoWrite: 'task'
};

export const family = (name: string): Family =>
	FAMILIES[name] ?? (name.startsWith('mcp__') ? 'plugin' : 'other');

export const ICONS: Record<Family, Component> = {
	shell: TerminalIcon,
	read: FileTextIcon,
	edit: FilePenIcon,
	search: SearchIcon,
	web: GlobeIcon,
	agent: BotIcon,
	plugin: PlugIcon,
	ask: MessageSquareIcon,
	task: ListChecksIcon,
	other: WrenchIcon
};

/** Tools whose consecutive calls read as one row. */
const FOLDABLE: Record<string, (n: number) => string> = {
	Read: (n) => `Read ${plural(n, 'file')}`,
	Edit: (n) => `Edited ${plural(n, 'file')}`,
	Write: (n) => `Wrote ${plural(n, 'file')}`,
	Grep: (n) => `Searched ${plural(n, 'pattern')}`,
	Glob: (n) => `Listed ${plural(n, 'pattern')}`
};

/** How a step reads: a sentence, a code part after it, a dim line below. */
export type Told = {
	text: string;
	/** Rendered in monospace after `text`: a path, a pattern, a command. */
	code?: string;
	/** A second, dim line: the command behind a description, the files of a fold. */
	detail?: string;
	/** A failed call's error, first line. */
	error?: string;
};

/** The last path component. */
export const base = (path: string) => path.split('/').pop() || path;

/** `https://docs.ntfy.sh/publish/` → `docs.ntfy.sh/publish`. */
const site = (url: string) => url.replace(/^https?:\/\//, '').replace(/\/$/, '');

/** `mcp__plugin_hm_playwright__browser_click` → `['playwright', 'click']`. */
export function plugin(name: string): [string, string] | null {
	const m = name.match(/^mcp__(?:plugin_)?(?:[a-z0-9]+_)?([a-z0-9-]+)__(.+)$/i);
	if (!m) return null;
	const action = m[2]
		.replace(/^browser_/, '')
		.replace(/_unsafe$/, '')
		.replace(/_/g, ' ');
	return [m[1], action];
}

const firstLine = (text: string | undefined) => text?.trim().split('\n')[0] ?? '';

/** What `tell` reads of a call. */
export type Call = Pick<Tool, 'name' | 'summary' | 'ok'> & {
	description?: string | null;
	output?: string | null;
};

/** One tool call as a sentence. */
export function tell(t: Call): Told {
	const s = t.summary;
	const told = ((): Told => {
		switch (t.name) {
			case 'Bash':
				if (t.description) return { text: t.description, detail: s };
				return s ? { text: '', code: s } : { text: 'Ran a command' };
			case 'Read':
				return { text: 'Read', code: s };
			case 'Edit':
			case 'MultiEdit':
			case 'NotebookEdit':
				return { text: 'Edited', code: s };
			case 'Write':
				return { text: 'Wrote', code: s };
			case 'Grep':
				return { text: 'Searched for', code: s };
			case 'Glob':
				return { text: 'Listed', code: s };
			case 'ToolSearch':
				return { text: 'Looked up tools', detail: s };
			case 'WebFetch':
				return { text: 'Fetched', code: site(s) };
			case 'WebSearch':
				return { text: 'Searched the web for', code: s };
			case 'Agent':
			case 'Task':
				return { text: s ? `Subagent: ${s}` : 'Started a subagent' };
			case 'SendMessage':
				return { text: 'Messaged a subagent' };
			case 'TaskStop':
				return { text: 'Stopped a background task' };
			case 'Monitor':
				return { text: s ? `Watching: ${s}` : 'Started a monitor' };
			case 'Skill':
				return { text: 'Loaded skill', code: s };
			case 'AskUserQuestion':
				return { text: s ? `Asked: ${s}` : 'Asked you a question' };
			case 'ExitPlanMode':
				return { text: 'Proposed a plan' };
			case 'EnterPlanMode':
				return { text: 'Started planning' };
			case 'TaskCreate':
				return { text: s ? `Added task: ${s}` : 'Added a task' };
			case 'TaskUpdate':
				return { text: s ? `Task ${s}` : 'Updated a task' };
			case 'TodoWrite':
				return { text: 'Updated the task list' };
		}
		const p = plugin(t.name);
		if (p) return { text: `${p[0]}: ${p[1]}`, code: s || undefined };
		return { text: toolName(t.name), code: s || undefined };
	})();
	if (!t.ok) told.error = firstLine(t.output ?? undefined) || 'failed';
	return told;
}

/** A session's recent step, told as the chat tells a call. */
export const tellStep = (step: Step) =>
	tell({ ...step, ok: step.error === null, output: step.error });

export type StepRow = {
	kind: 'step';
	key: string;
	at: number;
	ms: number | null;
	family: Family;
	item: Tool;
	told: Told;
};

export type FoldRow = {
	kind: 'fold';
	key: string;
	at: number;
	ms: number | null;
	family: Family;
	items: Tool[];
	told: Told;
};

export type OtherRow = { kind: 'other'; key: string; at: number; item: Exclude<Item, Tool> };

export type Row = StepRow | FoldRow | OtherRow;

const isTool = (i: Item): i is Tool => i.kind === 'tool';

/** The rows of a run: consecutive successful calls of one foldable tool
 * become one row listing what they touched. */
export function rows(items: Item[]): Row[] {
	const out: Row[] = [];
	let i = 0;
	while (i < items.length) {
		const item = items[i];
		if (!isTool(item)) {
			out.push({ kind: 'other', key: `o${item.id}`, at: item.at, item });
			i++;
			continue;
		}
		let j = i + 1;
		if (item.ok && FOLDABLE[item.name]) {
			while (j < items.length) {
				const next = items[j];
				if (!isTool(next) || next.name !== item.name || !next.ok) break;
				j++;
			}
		}
		const run = items.slice(i, j) as Tool[];
		if (run.length > 1) {
			const seen = new Set<string>();
			const names = run.map((t) => t.summary).filter((s) => s && !seen.has(s) && seen.add(s));
			out.push({
				kind: 'fold',
				key: `f${item.id}`,
				at: item.at,
				ms: run.reduce<number | null>((sum, t) => (t.ms === null ? sum : (sum ?? 0) + t.ms), null),
				family: family(item.name),
				items: run,
				told: {
					text: FOLDABLE[item.name](run.length),
					detail: names.map(base).join(', ')
				}
			});
		} else {
			out.push({
				kind: 'step',
				key: `s${item.id}`,
				at: item.at,
				ms: item.ms,
				family: family(item.name),
				item,
				told: tell(item)
			});
		}
		i = j;
	}
	return out;
}

const COUNTED: Record<Family, (n: number) => string> = {
	shell: (n) => `ran ${plural(n, 'command')}`,
	read: (n) => `read ${plural(n, 'file')}`,
	edit: (n) => `edited ${plural(n, 'file')}`,
	search: (n) => plural(n, 'search', 'searches'),
	web: (n) => plural(n, 'web call'),
	agent: (n) => plural(n, 'subagent call'),
	plugin: (n) => plural(n, 'plugin call'),
	ask: (n) => plural(n, 'question'),
	task: (n) => plural(n, 'task update'),
	other: (n) => plural(n, 'other call')
};
const ORDER: Family[] = [
	'shell',
	'edit',
	'read',
	'search',
	'web',
	'agent',
	'plugin',
	'ask',
	'task',
	'other'
];

/** The run in one line: "Ran 9 commands, edited 2 files · 1 failed · 2m". */
export function summarize(items: Item[], omitted = 0): string {
	const tools = items.filter(isTool);
	const counts = new Map<Family, Set<string> | number>();
	for (const t of tools) {
		const f = family(t.name);
		if (f === 'edit' || f === 'read') {
			const set = (counts.get(f) as Set<string> | undefined) ?? new Set<string>();
			set.add(t.summary || String(t.id));
			counts.set(f, set);
		} else counts.set(f, ((counts.get(f) as number | undefined) ?? 0) + 1);
	}
	const parts = ORDER.filter((f) => counts.has(f)).map((f) => {
		const c = counts.get(f)!;
		return COUNTED[f](typeof c === 'number' ? c : c.size);
	});
	if (omitted) parts.push(`${plural(omitted, 'earlier call')} not shown`);
	if (!parts.length) parts.push('no tool calls');
	const head = parts.join(', ');
	const failed = tools.filter((t) => !t.ok).length;
	const asks = items.filter((i) => i.kind === 'permission').length;
	const span = items.length ? items[items.length - 1].at - items[0].at : 0;
	return [
		head.charAt(0).toUpperCase() + head.slice(1),
		failed ? `${failed} failed` : null,
		asks ? plural(asks, 'permission request') : null,
		span >= 1000 ? fmtDuration(span) : null
	]
		.filter(Boolean)
		.join(' · ');
}
