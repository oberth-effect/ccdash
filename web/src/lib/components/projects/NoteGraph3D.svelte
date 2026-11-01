<script lang="ts">
	// The note graph in 3D: `3d-force-graph` over WebGL, fetched the first
	// time it is asked for, as three.js is most of a megabyte. It takes the
	// same data as `NoteGraph`. Drag rotates, the wheel zooms, right drag
	// pans; a click selects and a double click opens. `onfail` says why it
	// cannot draw (no WebGL, or the chunk would not load).
	import type { GraphEdge, GraphNode, LinkKind } from '#lib/api/types.js';
	import { degree, edgeKey, type Adjacency } from '#lib/graph.js';
	import { KIND_LABEL } from '#lib/notes.js';
	import type { GraphPhysics } from '#lib/prefs.svelte.js';

	type N = {
		id: number;
		node: GraphNode;
		x?: number;
		y?: number;
		z?: number;
		vx?: number;
		vy?: number;
		vz?: number;
	};
	type L = { source: number | N; target: number | N; kind: LinkKind };
	/** The library's instance, typed as far as this file uses it. */
	type Graph3D = {
		graphData(data: { nodes: N[]; links: L[] }): Graph3D;
		width(px: number): Graph3D;
		height(px: number): Graph3D;
		backgroundColor(colour: string): Graph3D;
		nodeColor(of: (n: N) => string): Graph3D;
		nodeVal(of: (n: N) => number): Graph3D;
		nodeLabel(of: (n: N) => HTMLElement): Graph3D;
		nodeOpacity(value: number): Graph3D;
		nodeResolution(value: number): Graph3D;
		showNavInfo(value: boolean): Graph3D;
		nodeThreeObject(of: (n: N) => unknown): Graph3D;
		nodeThreeObjectExtend(value: boolean): Graph3D;
		linkColor(of: (l: L) => string): Graph3D;
		linkOpacity(value: number): Graph3D;
		linkWidth(of: (l: L) => number): Graph3D;
		linkDirectionalParticles(of: (l: L) => number): Graph3D;
		linkDirectionalParticleWidth(value: number): Graph3D;
		onNodeClick(fn: (n: N) => void): Graph3D;
		onBackgroundClick(fn: () => void): Graph3D;
		cameraPosition(at?: object, lookAt?: object, ms?: number): { x: number; y: number; z: number };
		zoomToFit(ms?: number, padding?: number): Graph3D;
		d3Force(name: string): { strength(v: number): void; distance(v: number): void } | undefined;
		d3Force(name: string, force: unknown): Graph3D;
		d3ReheatSimulation(): Graph3D;
		_destructor(): void;
	};

	/** Notes that carry a title, the best connected first. */
	const TITLES = 40;
	const FLIGHT_MS = 700;
	/** How far from a note the camera stops: closer, its neighbours fill the view. */
	const FLIGHT_DISTANCE = 240;
	const SPHERE_SEGMENTS = 20;
	const CLICK_MS = 280;
	/** The library's radius follows the cube root of this. */
	const VAL_PER_PX = 0.35;

	let {
		nodes,
		edges,
		adj,
		colour,
		radius,
		lit = null,
		path = null,
		selected = null,
		physics,
		onselect,
		onopen,
		onfail
	}: {
		nodes: GraphNode[];
		edges: GraphEdge[];
		adj: Adjacency;
		colour: (node: GraphNode) => string;
		radius: (node: GraphNode) => number;
		lit?: Set<number> | null;
		path?: Set<string> | null;
		selected?: number | null;
		physics: GraphPhysics;
		onselect: (node: GraphNode | null) => void;
		onopen: (node: GraphNode) => void;
		onfail: (why: string) => void;
	} = $props();

	let host = $state<HTMLDivElement | null>(null);
	let graph = $state.raw<Graph3D | null>(null);
	let loading = $state(true);
	let Sprite: (new (text: string) => SpriteLabel) | null = null;
	type SpriteLabel = {
		color: string;
		textHeight: number;
		backgroundColor: string | false;
		padding: number;
		material: { depthWrite: boolean };
		center: { y: number; set(x: number, y: number): void };
	};

	/** Positions survive filter changes: the library keeps them on these. */
	const placed: Record<number, N> = {};
	let lastClick = { id: -1, at: 0 };
	/** Bumped when the theme changes, to read the colours again. */
	let palette = $state(0);

	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = 1;
	const pen = canvas.getContext('2d', { willReadFrequently: true });
	const probe = document.createElement('i');

	/** A CSS colour as rgba(): WebGL knows neither `var()` nor `oklch()`. */
	function resolved(css: string, alpha = 1): string {
		probe.style.color = css;
		document.body.append(probe);
		const value = getComputedStyle(probe).color;
		probe.remove();
		if (!pen) return value;
		pen.clearRect(0, 0, 1, 1);
		pen.fillStyle = value;
		pen.fillRect(0, 0, 1, 1);
		const [r, g, b] = pen.getImageData(0, 0, 1, 1).data;
		return `rgba(${r},${g},${b},${alpha})`;
	}

	function tooltip(n: N): HTMLElement {
		// NOTE: built with text nodes: the library inserts a string as HTML, and titles are data
		const box = document.createElement('div');
		box.style.cssText =
			'font:12px system-ui;padding:4px 8px;border-radius:6px;background:rgba(0,0,0,.75);color:#fff;max-width:18rem';
		const title = document.createElement('b');
		title.textContent = n.node.title;
		const facts = document.createElement('div');
		facts.textContent = `${KIND_LABEL[n.node.kind]}${n.node.project ? ` · ${n.node.project}` : ''} · ${degree(adj, n.id)} links`;
		box.append(title, facts);
		return box;
	}

	const endId = (end: number | N) => (typeof end === 'object' ? end.id : end);

	/** A pull towards the middle: without one loose notes drift off, and
	 * fitting them leaves everything else a speck. */
	function gravity(strength: number) {
		let nodes: N[] = [];
		const force = (alpha: number) => {
			for (const n of nodes) {
				n.vx = (n.vx ?? 0) - (n.x ?? 0) * strength * alpha;
				n.vy = (n.vy ?? 0) - (n.y ?? 0) * strength * alpha;
				n.vz = (n.vz ?? 0) - (n.z ?? 0) * strength * alpha;
			}
		};
		force.initialize = (list: N[]) => {
			nodes = list;
		};
		return force;
	}

	const titled = $derived(
		new Set(
			[...nodes]
				.filter((n) => n.id === selected || !lit || lit.has(n.id))
				.sort((a, b) => degree(adj, b.id) - degree(adj, a.id))
				.slice(0, TITLES)
				.map((n) => n.id)
		)
	);

	function label(n: N) {
		if (!Sprite || !titled.has(n.id)) return null;
		const sprite = new Sprite(
			n.node.title.length > 32 ? `${n.node.title.slice(0, 31)}…` : n.node.title
		);
		sprite.color = resolved('var(--foreground)');
		sprite.textHeight = 3;
		sprite.backgroundColor = resolved('var(--card)', 0.6);
		sprite.padding = 0.8;
		sprite.material.depthWrite = false;
		sprite.center.y = -0.8;
		return sprite;
	}

	function paint(g: Graph3D) {
		g.backgroundColor(resolved('var(--card)'))
			.nodeColor((n) => resolved(colour(n.node), lit && !lit.has(n.id) ? 0.1 : 1))
			.nodeThreeObject(label)
			.linkColor((l) =>
				path?.has(edgeKey(endId(l.source), endId(l.target)))
					? resolved('var(--primary)')
					: resolved('var(--muted-foreground)')
			)
			.linkWidth((l) => (path?.has(edgeKey(endId(l.source), endId(l.target))) ? 1.2 : 0))
			.linkDirectionalParticles((l) =>
				path?.has(edgeKey(endId(l.source), endId(l.target))) ? 4 : 0
			);
	}

	$effect(() => {
		const el = host;
		if (!el) return;
		let gone = false;
		let observer: ResizeObserver | null = null;
		let instance: Graph3D | null = null;
		(async () => {
			try {
				if (!document.createElement('canvas').getContext('webgl2')) {
					throw new Error('this browser has no WebGL');
				}
				const [{ default: ForceGraph3D }, { default: SpriteText }] = await Promise.all([
					import('3d-force-graph'),
					import('three-spritetext')
				]);
				if (gone) return;
				Sprite = SpriteText as unknown as typeof Sprite;
				instance = new (ForceGraph3D as unknown as new (el: HTMLElement) => Graph3D)(el);
				instance
					.nodeVal((n) => radius(n.node) * VAL_PER_PX)
					.nodeLabel(tooltip)
					.nodeOpacity(0.95)
					.nodeResolution(SPHERE_SEGMENTS)
					.showNavInfo(false)
					.nodeThreeObjectExtend(true)
					.linkOpacity(0.3)
					.linkDirectionalParticleWidth(1.6)
					.onNodeClick((n) => {
						const now = performance.now();
						const again = lastClick.id === n.id && now - lastClick.at < CLICK_MS;
						lastClick = { id: n.id, at: now };
						if (again) onopen(n.node);
						else onselect(n.node);
					})
					.onBackgroundClick(() => onselect(null));
				observer = new ResizeObserver(() =>
					instance?.width(el.clientWidth).height(el.clientHeight)
				);
				observer.observe(el);
				graph = instance;
				loading = false;
			} catch (e) {
				if (!gone) onfail((e as Error).message);
			}
		})();
		return () => {
			gone = true;
			observer?.disconnect();
			instance?._destructor();
			graph = null;
			el.replaceChildren();
		};
	});

	// A scheme can change under the open graph.
	$effect(() => {
		const watch = new MutationObserver(() => palette++);
		watch.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['class', 'style', 'data-scheme']
		});
		return () => watch.disconnect();
	});

	$effect(() => {
		const g = graph;
		if (!g) return;
		const ns = nodes.map((node) => {
			const n = (placed[node.id] ??= { id: node.id, node });
			n.node = node;
			return n;
		});
		g.graphData({
			nodes: ns,
			links: edges.map((e) => ({ source: e.src, target: e.dst, kind: e.kind }))
		});
	});

	$effect(() => {
		const g = graph;
		if (!g) return;
		void palette;
		void lit;
		void path;
		void selected;
		void titled;
		void colour;
		paint(g);
	});

	$effect(() => {
		const g = graph;
		if (!g) return;
		g.d3Force('charge')?.strength(-physics.repulsion * 2);
		g.d3Force('link')?.distance(physics.distance + 10);
		g.d3Force('gravity', gravity(physics.gravity));
		g.d3ReheatSimulation();
	});

	/** Flies to a note. */
	export function reveal(id: number) {
		const n = placed[id];
		if (!graph || n?.x === undefined) return;
		const scale = 1 + FLIGHT_DISTANCE / Math.max(1, Math.hypot(n.x, n.y ?? 0, n.z ?? 0));
		graph.cameraPosition(
			{ x: n.x * scale, y: (n.y ?? 0) * scale, z: (n.z ?? 0) * scale },
			{ x: n.x, y: n.y, z: n.z },
			FLIGHT_MS
		);
	}

	export function zoomBy(factor: number) {
		if (!graph) return;
		const at = graph.cameraPosition();
		graph.cameraPosition(
			{ x: at.x / factor, y: at.y / factor, z: at.z / factor },
			undefined,
			FLIGHT_MS / 3
		);
	}

	export function fit() {
		graph?.zoomToFit(FLIGHT_MS, 40);
	}
</script>

<div bind:this={host} class="absolute inset-0"></div>
{#if loading}
	<p class="absolute inset-0 grid place-items-center text-sm text-muted-foreground">
		Loading the 3D view…
	</p>
{/if}
