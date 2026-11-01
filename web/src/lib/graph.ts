// What the note graph computes: who is linked to whom, a note's surroundings,
// the way between two notes, the isolated notes and the separate parts, the
// colour and size scales, and which titles fit on screen. No DOM here, so
// the views (2D and 3D) share it and it runs under `node` alone.

import type { Graph, GraphEdge, GraphNode, LinkKind, NoteKind } from './api/types';

export type Adjacency = Map<number, Set<number>>;

/** Links as an undirected graph: to explore, a link is a link either way. */
export function adjacency(edges: Pick<GraphEdge, 'src' | 'dst'>[]): Adjacency {
	const adj: Adjacency = new Map();
	const add = (a: number, b: number) => {
		let set = adj.get(a);
		if (!set) adj.set(a, (set = new Set()));
		set.add(b);
	};
	for (const { src, dst } of edges) {
		if (src === dst) continue;
		add(src, dst);
		add(dst, src);
	}
	return adj;
}

export const degree = (adj: Adjacency, id: number) => adj.get(id)?.size ?? 0;

/** `start` and everything within `depth` links of it. */
export function neighbourhood(start: number, depth: number, adj: Adjacency): Set<number> {
	const seen = new Set([start]);
	let ring = [start];
	for (let d = 0; d < depth && ring.length; d++) {
		const next: number[] = [];
		for (const id of ring) {
			for (const other of adj.get(id) ?? []) {
				if (!seen.has(other)) {
					seen.add(other);
					next.push(other);
				}
			}
		}
		ring = next;
	}
	return seen;
}

/** The fewest links from `from` to `to`, both ends included; null if none. */
export function shortestPath(from: number, to: number, adj: Adjacency): number[] | null {
	if (from === to) return [from];
	const came = new Map<number, number>([[from, from]]);
	let ring = [from];
	while (ring.length) {
		const next: number[] = [];
		for (const id of ring) {
			for (const other of adj.get(id) ?? []) {
				if (came.has(other)) continue;
				came.set(other, id);
				if (other === to) {
					const path = [to];
					while (path[0] !== from) path.unshift(came.get(path[0])!);
					return path;
				}
				next.push(other);
			}
		}
		ring = next;
	}
	return null;
}

/** The separate parts of the graph, largest first. */
export function components(ids: number[], adj: Adjacency): number[][] {
	const seen = new Set<number>();
	const parts: number[][] = [];
	for (const id of ids) {
		if (seen.has(id)) continue;
		const part = [...neighbourhood(id, Infinity, adj)];
		for (const member of part) seen.add(member);
		parts.push(part);
	}
	return parts.sort((a, b) => b.length - a.length);
}

/** An undirected key for the link between two notes. */
export const edgeKey = (a: number, b: number) => (a < b ? `${a}-${b}` : `${b}-${a}`);

/** The links along a path, as `edgeKey`s. */
export function pathEdges(path: number[]): Set<string> {
	return new Set(path.slice(1).map((id, i) => edgeKey(path[i], id)));
}

// --- scales ------------------------------------------------------------------

/** Series colours for projects, in order; the rest fold into `OTHER`. They
 * are the raw theme variables, so every scheme recolours them. */
const SERIES = [
	'var(--working)',
	'var(--permission)',
	'var(--your-turn)',
	'var(--series-4)',
	'var(--series-5)',
	'var(--series-6)'
];
const OTHER = 'var(--muted-foreground)';
const RAMP = [1, 2, 3, 4, 5, 6].map((step) => `var(--seq-${step})`);

export type Swatch = { label: string; colour: string };

/**
 * A colour per project, the biggest first, from every note of the workspace
 * rather than the filtered ones, so filtering never repaints a project.
 */
export function projectColours(nodes: GraphNode[]): { of: Map<string, string>; legend: Swatch[] } {
	const counts = new Map<string, number>();
	for (const n of nodes) counts.set(n.project ?? '', (counts.get(n.project ?? '') ?? 0) + 1);
	const names = [...counts.keys()].sort(
		(a, b) => counts.get(b)! - counts.get(a)! || a.localeCompare(b)
	);
	const of = new Map(names.map((name, i) => [name, SERIES[i] ?? OTHER]));
	const legend = names
		.slice(0, SERIES.length)
		.map((name, i) => ({ label: name || 'Workspace-wide', colour: SERIES[i] }));
	if (names.length > SERIES.length) legend.push({ label: 'Other', colour: OTHER });
	return { of, legend };
}

/** The ramp's colour for a value: six steps by rank, so one huge outlier
 * does not leave everything else in the first step. */
export function rankScale(values: number[]): (value: number) => string {
	const sorted = [...values].sort((a, b) => a - b);
	if (!sorted.length) return () => RAMP[0];
	return (value) => {
		let low = 0;
		let high = sorted.length;
		while (low < high) {
			const mid = (low + high) >> 1;
			if (sorted[mid] < value) low = mid + 1;
			else high = mid;
		}
		return RAMP[Math.min(RAMP.length - 1, Math.floor((low / sorted.length) * RAMP.length))];
	};
}

// --- labels ------------------------------------------------------------------

/** A title that may be drawn, with its anchor on screen. */
export type LabelCandidate = {
	id: number;
	x: number;
	y: number;
	text: string;
	/** Higher goes first. */
	rank: number;
	/** Always drawn: the hovered or selected note. */
	must?: boolean;
};

const CHAR_PX = 5.4;
const LINE_PX = 13;
const CELL_PX = 64;

/**
 * Which titles to draw so that none overlap: greedily by rank, on a grid of
 * cells so it stays linear in the number of notes. Anchors are centred above
 * the note; those off screen are skipped.
 */
export function placeLabels(
	candidates: LabelCandidate[],
	width: number,
	height: number,
	max: number
): Set<number> {
	const grid = new Map<number, [number, number, number, number][]>();
	const cells = (x0: number, y0: number, x1: number, y1: number) => {
		const out: number[] = [];
		for (let cx = Math.floor(x0 / CELL_PX); cx <= Math.floor(x1 / CELL_PX); cx++) {
			for (let cy = Math.floor(y0 / CELL_PX); cy <= Math.floor(y1 / CELL_PX); cy++) {
				out.push(cx * 100_003 + cy);
			}
		}
		return out;
	};
	const shown = new Set<number>();
	const ordered = [...candidates].sort(
		(a, b) => Number(b.must ?? false) - Number(a.must ?? false) || b.rank - a.rank
	);
	for (const c of ordered) {
		if (!c.must && shown.size >= max) break;
		const half = (c.text.length * CHAR_PX) / 2;
		const box: [number, number, number, number] = [c.x - half, c.y - LINE_PX, c.x + half, c.y];
		const inView = box[2] > 0 && box[0] < width && box[3] > 0 && box[1] < height;
		if (!inView && !c.must) continue;
		const keys = cells(box[0], box[1], box[2], box[3]);
		const hits = keys.some((key) =>
			grid.get(key)?.some((b) => box[0] < b[2] && box[2] > b[0] && box[1] < b[3] && box[3] > b[1])
		);
		if (hits && !c.must) continue;
		for (const key of keys) {
			const bucket = grid.get(key);
			if (bucket) bucket.push(box);
			else grid.set(key, [box]);
		}
		shown.add(c.id);
	}
	return shown;
}

// --- what is shown -----------------------------------------------------------

export type OrphanFilter = 'all' | 'hide' | 'only';

export type GraphFilter = {
	kinds: Record<NoteKind, boolean>;
	/** A project, `''` for workspace-wide notes; null is every project. */
	project: string | null;
	/** Notes created after this are not there yet; null is all of them. */
	until: number | null;
	links: Record<LinkKind, boolean>;
	orphans: OrphanFilter;
	/** The local graph: only what is within `depth` links of this note. */
	focus: number | null;
	depth: number;
	/** With `only`, just these notes and their links. */
	matches: Set<number> | null;
	only: boolean;
};

export type Shown = { nodes: GraphNode[]; edges: GraphEdge[]; adj: Adjacency };

/**
 * Applies the filters in the order they mean something: what exists at all
 * (kind, project, time, link kinds), then the local graph within that, then
 * isolated notes, which are judged by the links left, then the search.
 */
export function shownGraph(graph: Graph, filter: GraphFilter): Shown {
	let nodes = graph.nodes.filter(
		(n) =>
			filter.kinds[n.kind] &&
			(filter.project === null || (n.project ?? '') === filter.project) &&
			(filter.until === null || n.created_at <= filter.until)
	);
	const within = (list: GraphEdge[], ids: Set<number>) =>
		list.filter((e) => ids.has(e.src) && ids.has(e.dst));
	let edges = within(
		graph.edges.filter((e) => filter.links[e.kind]),
		new Set(nodes.map((n) => n.id))
	);
	if (filter.focus !== null && nodes.some((n) => n.id === filter.focus)) {
		const near = neighbourhood(filter.focus, filter.depth, adjacency(edges));
		nodes = nodes.filter((n) => near.has(n.id));
		edges = within(edges, near);
	}
	if (filter.orphans !== 'all') {
		const adj = adjacency(edges);
		const wanted = filter.orphans === 'only';
		nodes = nodes.filter((n) => n.id === filter.focus || (degree(adj, n.id) === 0) === wanted);
		edges = within(edges, new Set(nodes.map((n) => n.id)));
	}
	if (filter.only && filter.matches) {
		const { matches } = filter;
		nodes = nodes.filter((n) => matches.has(n.id));
		edges = within(edges, matches);
	}
	return { nodes, edges, adj: adjacency(edges) };
}

export const indexById = (nodes: GraphNode[]) => new Map(nodes.map((n) => [n.id, n]));

export const slugIndex = (nodes: GraphNode[]) => new Map(nodes.map((n) => [n.slug, n.id]));
