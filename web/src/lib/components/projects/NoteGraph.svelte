<script lang="ts">
	// The note graph in 2D: a d3 force layout drawn in SVG. What it shows and
	// emphasises comes from `GraphTab`; this draws it and takes the gestures.
	// Wheel or two fingers zoom, dragging the background pans, dragging a
	// note moves it, a click selects it and a double click opens it. Titles
	// go where they fit, the best connected first (`placeLabels`).
	import { untrack } from 'svelte';
	import {
		forceCenter,
		forceCollide,
		forceLink,
		forceManyBody,
		forceSimulation,
		forceX,
		forceY,
		type Simulation,
		type SimulationLinkDatum,
		type SimulationNodeDatum
	} from 'd3-force';
	import type { GraphEdge, GraphNode, LinkKind } from '#lib/api/types.js';
	import { fmtDate, plural } from '#lib/format.js';
	import { degree, edgeKey, placeLabels, type Adjacency } from '#lib/graph.js';
	import { KIND_LABEL } from '#lib/notes.js';
	import type { GraphLayout, GraphPhysics } from '#lib/prefs.svelte.js';

	type N = SimulationNodeDatum & { id: number; node: GraphNode; r: number };
	type L = SimulationLinkDatum<N> & { kind: LinkKind };
	type Dot = { id: number; x: number; y: number; r: number; node: GraphNode };
	type Line = {
		key: string;
		a: number;
		b: number;
		x1: number;
		y1: number;
		x2: number;
		y2: number;
		kind: LinkKind;
	};

	/** Titles drawn at once, at most. */
	const MAX_LABELS = 120;
	const CLICK_MS = 280;
	/** Less than this between press and release is a click, not a drag. */
	const CLICK_PX = 4;
	/** Timeline width in graph units per screen width, so it can be explored. */
	const TIMELINE_STRETCH = 1.6;
	const CLUSTER_RING = 0.42;
	const REVEAL_MS = 350;

	let {
		nodes,
		edges,
		adj,
		colour,
		radius,
		lit = null,
		path = null,
		selected = null,
		layout = 'force',
		physics,
		onselect,
		onopen
	}: {
		nodes: GraphNode[];
		edges: GraphEdge[];
		adj: Adjacency;
		colour: (node: GraphNode) => string;
		radius: (node: GraphNode) => number;
		/** The notes to keep bright; everything else dims. Null: none dim. */
		lit?: Set<number> | null;
		/** The links of a found path. */
		path?: Set<string> | null;
		selected?: number | null;
		layout?: GraphLayout;
		physics: GraphPhysics;
		onselect: (node: GraphNode | null) => void;
		onopen: (node: GraphNode) => void;
	} = $props();

	let width = $state(600);
	let height = $state(400);
	let view = $state({ k: 1, x: 0, y: 0 });
	let dots = $state.raw<Dot[]>([]);
	let lines = $state.raw<Line[]>([]);
	let hovered = $state<Dot | null>(null);
	let svg = $state<SVGSVGElement | null>(null);

	let sim: Simulation<N, L> | null = null;
	/** Positions survive filter changes. */
	const placed: Record<number, N> = {};
	let fitted = false;

	/** A centre per project, on a ring, for the Clusters layout. */
	function clusterCentres(ns: N[]): Record<string, [number, number]> {
		const names = [...new Set(ns.map((n) => n.node.project ?? ''))].sort();
		const size = Math.min(width, height) * CLUSTER_RING;
		return Object.fromEntries(
			names.map((name, i) => {
				const angle = (i / names.length) * Math.PI * 2;
				return [name, [width / 2 + Math.cos(angle) * size, height / 2 + Math.sin(angle) * size]];
			})
		);
	}

	function rebuild() {
		sim?.stop();
		// a changed node set fits itself once more, unless the user pans or zooms first
		fitted = false;
		const ns: N[] = nodes.map((node) => {
			const old = placed[node.id];
			const n: N = old
				? { ...old, node, r: radius(node), fx: null, fy: null }
				: { id: node.id, node, r: radius(node) };
			placed[node.id] = n;
			return n;
		});
		const ids = new Set(ns.map((n) => n.id));
		const ls: L[] = edges
			.filter((e) => ids.has(e.src) && ids.has(e.dst))
			.map((e) => ({ source: e.src, target: e.dst, kind: e.kind }));
		sim = forceSimulation<N, L>(ns)
			.force(
				'link',
				forceLink<N, L>(ls)
					.id((d) => d.id)
					.distance((l) => physics.distance + (l.source as N).r + (l.target as N).r)
					.strength(layout === 'force' ? 0.6 : 0.15)
			)
			.force('charge', forceManyBody<N>().strength(-physics.repulsion).distanceMax(250))
			.force('center', forceCenter(width / 2, height / 2).strength(0.05))
			// NOTE: `forceCenter` only moves the mean: this is what keeps loose notes from drifting off
			.force('x', forceX<N>(width / 2).strength(physics.gravity))
			.force('y', forceY<N>(height / 2).strength(physics.gravity))
			.force(
				'collide',
				forceCollide<N>((d) => d.r + 3)
			);
		if (layout === 'clusters') {
			const centres = clusterCentres(ns);
			sim
				.force('x', forceX<N>((d) => centres[d.node.project ?? ''][0]).strength(0.25))
				.force('y', forceY<N>((d) => centres[d.node.project ?? ''][1]).strength(0.25));
		} else if (layout === 'timeline') {
			const times = ns.map((n) => n.node.created_at);
			const [from, to] = [Math.min(...times), Math.max(...times)];
			const stretch = width * TIMELINE_STRETCH;
			sim
				.force(
					'x',
					forceX<N>(
						(d) => ((d.node.created_at - from) / Math.max(1, to - from)) * stretch
					).strength(0.9)
				)
				.force('y', forceY<N>(height / 2).strength(0.06))
				.force('center', null);
		}
		sim
			.alpha(ns.some((n) => n.x === undefined) ? 1 : 0.5)
			.on('end', () => {
				if (!fitted) {
					fitted = true;
					fit();
				}
			})
			.on('tick', () => {
				dots = ns.map((n) => ({ id: n.id, x: n.x ?? 0, y: n.y ?? 0, r: n.r, node: n.node }));
				lines = ls.map((l) => {
					const s = l.source as N;
					const t = l.target as N;
					return {
						key: `${s.id}-${t.id}-${l.kind}`,
						a: s.id,
						b: t.id,
						x1: s.x ?? 0,
						y1: s.y ?? 0,
						x2: t.x ?? 0,
						y2: t.y ?? 0,
						kind: l.kind
					};
				});
			});
	}

	$effect(() => {
		void nodes;
		void edges;
		void layout;
		void physics.repulsion;
		void physics.distance;
		void physics.gravity;
		untrack(rebuild);
		return () => sim?.stop();
	});

	// A new size changes what "fit" means, as entering full screen does.
	$effect(() => {
		void width;
		void height;
		untrack(() => {
			if (fitted && !userMoved) fit();
		});
	});

	// --- camera --------------------------------------------------------------

	/** The user panned or zoomed: nothing re-fits behind their back. */
	let userMoved = false;
	let flight = 0;

	function animateTo(target: { k: number; x: number; y: number }) {
		cancelAnimationFrame(flight);
		const from = view;
		const start = performance.now();
		const step = (now: number) => {
			const t = Math.min(1, (now - start) / REVEAL_MS);
			const eased = t * (2 - t);
			view = {
				k: from.k + (target.k - from.k) * eased,
				x: from.x + (target.x - from.x) * eased,
				y: from.y + (target.y - from.y) * eased
			};
			if (t < 1) flight = requestAnimationFrame(step);
		};
		flight = requestAnimationFrame(step);
	}

	/** Brings a note to the middle, a little closer if the view is far out. */
	export function reveal(id: number) {
		const d = dots.find((dot) => dot.id === id);
		if (!d) return;
		userMoved = true;
		fitted = true;
		const k = Math.max(view.k, 1);
		animateTo({ k, x: width / 2 - d.x * k, y: height / 2 - d.y * k });
	}

	export function zoomBy(factor: number, at?: [number, number]) {
		fitted = true;
		userMoved = true;
		const k = Math.min(6, Math.max(0.1, view.k * factor));
		const [cx, cy] = at ?? [width / 2, height / 2];
		// keep the point under the cursor fixed
		view = { k, x: cx - ((cx - view.x) * k) / view.k, y: cy - ((cy - view.y) * k) / view.k };
	}

	export function fit() {
		userMoved = false;
		if (!dots.length) return void (view = { k: 1, x: 0, y: 0 });
		const xs = dots.map((d) => d.x);
		const ys = dots.map((d) => d.y);
		const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
		const k = Math.min(
			3,
			0.9 * Math.min(width / Math.max(40, x1 - x0), height / Math.max(40, y1 - y0))
		);
		view = { k, x: width / 2 - ((x0 + x1) / 2) * k, y: height / 2 - ((y0 + y1) / 2) * k };
	}

	/** A point in the SVG's own coordinates. */
	function local(event: { clientX: number; clientY: number }): [number, number] {
		const box = svg!.getBoundingClientRect();
		return [event.clientX - box.left, event.clientY - box.top];
	}

	$effect(() => {
		const el = svg;
		if (!el) return;
		const onwheel = (event: WheelEvent) => {
			event.preventDefault();
			zoomBy(event.deltaY < 0 ? 1.15 : 1 / 1.15, local(event));
		};
		// NOTE: wheel listeners are passive by default, which forbids preventDefault
		el.addEventListener('wheel', onwheel, { passive: false });
		return () => el.removeEventListener('wheel', onwheel);
	});

	// --- pointers ------------------------------------------------------------

	type Drag =
		| { kind: 'pan'; x: number; y: number; startX: number; startY: number }
		| { kind: 'node'; n: N; dot: Dot; moved: boolean }
		| { kind: 'pinch'; distance: number; at: [number, number] };

	let pointer = $state.raw<Drag | null>(null);
	/** The fingers down, for a pinch. */
	const fingers: Record<number, [number, number]> = {};
	let lastClick = { id: -1, at: 0 };

	const pinchOf = (): { distance: number; at: [number, number] } | null => {
		const [a, b] = Object.values(fingers);
		if (!a || !b) return null;
		return {
			distance: Math.hypot(a[0] - b[0], a[1] - b[1]),
			at: [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]
		};
	};

	function down(event: PointerEvent, dot?: Dot) {
		if (event.button !== 0) return;
		(event.currentTarget as Element).setPointerCapture(event.pointerId);
		fingers[event.pointerId] = local(event);
		const pinch = pinchOf();
		if (pinch) {
			// NOTE: a second finger takes over from whatever the first one started
			if (pointer?.kind === 'node') release(pointer.n);
			pointer = { kind: 'pinch', ...pinch };
			return;
		}
		if (dot) {
			const n = placed[dot.id];
			pointer = { kind: 'node', n, dot, moved: false };
			n.fx = n.x;
			n.fy = n.y;
			sim?.alphaTarget(0.3).restart();
		} else {
			fitted = true;
			pointer = {
				kind: 'pan',
				x: event.clientX - view.x,
				y: event.clientY - view.y,
				startX: event.clientX,
				startY: event.clientY
			};
		}
	}

	function move(event: PointerEvent) {
		if (event.pointerId in fingers) fingers[event.pointerId] = local(event);
		if (!pointer) return;
		if (pointer.kind === 'pinch') {
			const pinch = pinchOf();
			if (!pinch) return;
			view = {
				...view,
				x: view.x + pinch.at[0] - pointer.at[0],
				y: view.y + pinch.at[1] - pointer.at[1]
			};
			zoomBy(pinch.distance / Math.max(1, pointer.distance), pinch.at);
			pointer = { kind: 'pinch', ...pinch };
		} else if (pointer.kind === 'pan') {
			userMoved = true;
			view = { ...view, x: event.clientX - pointer.x, y: event.clientY - pointer.y };
		} else {
			const [x, y] = local(event);
			pointer.n.fx = (x - view.x) / view.k;
			pointer.n.fy = (y - view.y) / view.k;
			pointer.moved = true;
		}
	}

	function release(n: N) {
		n.fx = null;
		n.fy = null;
		sim?.alphaTarget(0);
	}

	function up(event: PointerEvent) {
		delete fingers[event.pointerId];
		if (!pointer) return;
		if (pointer.kind === 'pinch') {
			// the finger left behind must not pan from where the pinch began
			if (!pinchOf()) pointer = null;
			return;
		}
		if (pointer.kind === 'pan') {
			const travelled = Math.hypot(event.clientX - pointer.startX, event.clientY - pointer.startY);
			if (travelled < CLICK_PX) onselect(null);
		} else {
			release(pointer.n);
			if (!pointer.moved) {
				const now = performance.now();
				const again = lastClick.id === pointer.dot.id && now - lastClick.at < CLICK_MS;
				lastClick = { id: pointer.dot.id, at: now };
				if (again) onopen(pointer.dot.node);
				else onselect(pointer.dot.node);
			}
		}
		pointer = null;
	}

	// --- drawing -------------------------------------------------------------

	const dash: Record<LinkKind, string | undefined> = {
		mentions: undefined,
		relates: '3 3',
		supersedes: '6 3',
		blocks: undefined
	};

	/** What hovering adds to what is lit: the note and its neighbours. */
	const glow = $derived.by(() => {
		if (!hovered || pointer) return null;
		const around = adj.get(hovered.id);
		return new Set([hovered.id, ...(around ?? [])]);
	});
	// NOTE: a found path stays what is lit: it was hard enough to get
	const bright = $derived(path ? lit : (glow ?? lit));
	const opacityOf = (d: Dot) => {
		if (bright && !bright.has(d.id)) return 0.12;
		return d.node.status === 'archived' || d.node.status === 'superseded' ? 0.45 : 1;
	};
	const lineOpacity = (l: Line) => {
		if (path?.has(edgeKey(l.a, l.b))) return 1;
		if (bright) return bright.has(l.a) && bright.has(l.b) ? 0.8 : 0.05;
		return l.kind === 'mentions' ? 0.3 : 0.6;
	};
	const lineColour = (l: Line) => {
		if (path?.has(edgeKey(l.a, l.b))) return 'var(--primary)';
		return l.kind === 'blocks' ? 'var(--critical)' : 'var(--muted-foreground)';
	};
	const lineWidth = (l: Line) =>
		path?.has(edgeKey(l.a, l.b)) ? 3 : l.kind === 'supersedes' ? 2 : 1.2;

	const labelled = $derived(
		placeLabels(
			dots
				.filter(
					(d) =>
						d.id === hovered?.id ||
						d.id === selected ||
						(d.node.kind !== 'progress' && (!bright || bright.has(d.id))) ||
						(bright?.has(d.id) ?? false)
				)
				.map((d) => ({
					id: d.id,
					x: d.x * view.k + view.x,
					y: d.y * view.k + view.y - d.r - 3,
					text: titleOf(d),
					rank: degree(adj, d.id) * 2 + d.r + (bright?.has(d.id) ? 100 : 0),
					must: d.id === hovered?.id || d.id === selected
				})),
			width,
			height,
			MAX_LABELS
		)
	);
	function titleOf(d: Dot) {
		return d.node.title.length > 32 && d.id !== hovered?.id && d.id !== selected
			? `${d.node.title.slice(0, 31)}…`
			: d.node.title;
	}
	const screen = (d: Dot) => [d.x * view.k + view.x, d.y * view.k + view.y] as const;
</script>

<div class="absolute inset-0" bind:clientWidth={width} bind:clientHeight={height}>
	<svg
		bind:this={svg}
		{width}
		{height}
		class="touch-none select-none {pointer?.kind === 'pan' ? 'cursor-grabbing' : 'cursor-grab'}"
		role="img"
		aria-label="Note graph"
		onpointerdown={(e) => down(e)}
		onpointermove={move}
		onpointerup={up}
		onpointercancel={up}
	>
		<!-- NOTE: marks keep their size through `--k`, so a zoom touches one element -->
		<g transform="translate({view.x} {view.y}) scale({view.k})" style:--k={view.k}>
			{#each lines as l (l.key)}
				<line
					x1={l.x1}
					y1={l.y1}
					x2={l.x2}
					y2={l.y2}
					stroke={lineColour(l)}
					stroke-opacity={lineOpacity(l)}
					stroke-width={lineWidth(l)}
					stroke-dasharray={dash[l.kind]}
					vector-effect="non-scaling-stroke"
				/>
			{/each}
			{#each dots as d (d.id)}
				<g
					transform="translate({d.x} {d.y})"
					class="cursor-pointer"
					opacity={opacityOf(d)}
					role="button"
					tabindex="-1"
					aria-label={d.node.title}
					onpointerdown={(e) => {
						e.stopPropagation();
						down(e, d);
					}}
					onpointerenter={() => (hovered = d)}
					onpointerleave={() => (hovered = hovered?.id === d.id ? null : hovered)}
				>
					<g class="graph-mark">
						{#if d.id === selected}
							<circle r={d.r + 4} fill="none" stroke="var(--primary)" stroke-width="2"></circle>
						{/if}
						{#if d.node.kind === 'progress'}
							<rect
								x={-d.r}
								y={-d.r}
								width={d.r * 2}
								height={d.r * 2}
								transform="rotate(45)"
								fill={colour(d.node)}
								stroke="var(--card)"
								stroke-width="1.5"
							/>
						{:else}
							<circle
								r={d.r}
								fill={colour(d.node)}
								stroke="var(--card)"
								stroke-width="2"
								stroke-dasharray={d.node.status === 'stale' ? '2 2' : undefined}
							/>
						{/if}
						<circle r={Math.max(d.r, 10)} fill="transparent" />
					</g>
				</g>
			{/each}
			{#each dots as d (d.id)}
				{#if labelled.has(d.id)}
					<g transform="translate({d.x} {d.y})" class="pointer-events-none">
						<text
							y={-(d.r + 3)}
							text-anchor="middle"
							font-size="10"
							class="graph-mark fill-foreground"
							opacity={d.id === hovered?.id || d.id === selected ? 1 : 0.85}
							paint-order="stroke"
							stroke="var(--card)"
							stroke-width="3">{titleOf(d)}</text
						>
					</g>
				{/if}
			{/each}
		</g>
	</svg>
</div>
{#if hovered && !pointer}
	{@const [sx, sy] = screen(hovered)}
	<div
		class="pointer-events-none absolute z-10 max-w-64 rounded-md border bg-popover px-2 py-1.5 text-xs shadow"
		style="left: {Math.min(sx + 12, width - 260)}px; top: {Math.max(sy + 12, 0)}px"
	>
		<span class="font-medium">{hovered.node.title}</span>
		<span class="block text-muted-foreground"
			>{KIND_LABEL[hovered.node.kind]}{hovered.node.project
				? ` · ${hovered.node.project}`
				: ''}{hovered.node.status !== 'active' ? ` · ${hovered.node.status}` : ''}</span
		>
		<span class="block text-muted-foreground"
			>{plural(degree(adj, hovered.id), 'link')} · {plural(hovered.node.recalls, 'recall')} · created
			{fmtDate(hovered.node.created_at)}</span
		>
	</div>
{/if}

<style>
	.graph-mark {
		transform: scale(calc(1 / var(--k)));
	}
</style>
