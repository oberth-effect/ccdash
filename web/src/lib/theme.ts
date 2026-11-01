// The colour schemes. Every scheme but the two in `layout.css` is a `Base`,
// a theme's own few colours; a machine's wallpaper palette is turned into
// one, and `tokens` makes the panel's CSS variables out of a `Base`, keeping
// text legible on whatever it was given. No DOM here: `prefs` applies them.

import type { Palette, PaletteColours } from './api/types';

/** A theme's own colours, all `#rrggbb`. */
export type Base = {
	bg: string;
	/** Cards; a step off `bg` when the theme has none. */
	surface?: string;
	fg: string;
	/** Dim text; between `fg` and `bg` when the theme has none. */
	muted?: string;
	primary: string;
	red: string;
	orange: string;
	yellow: string;
	green: string;
	cyan: string;
	blue: string;
	magenta: string;
};

export type Scheme = {
	id: string;
	label: string;
	/** Defined in `layout.css`, not here. */
	css?: boolean;
	dark?: Base;
	light?: Base;
};

type Hues = [string, string, string, string, string, string, string];

/** `hues`: red, orange, yellow, green, cyan, blue, magenta. */
const base = (
	bg: string,
	surface: string,
	fg: string,
	muted: string,
	[red, orange, yellow, green, cyan, blue, magenta]: Hues,
	primary = blue
): Base => ({ bg, surface, fg, muted, primary, red, orange, yellow, green, cyan, blue, magenta });

export const WALLPAPER = 'wallpaper';

export const SCHEMES: Scheme[] = [
	{ id: 'neutral', label: 'Neutral', css: true },
	{ id: 'ccdash', label: 'ccdash', css: true },
	{
		id: 'catppuccin',
		label: 'Catppuccin',
		dark: base(
			'#1e1e2e',
			'#262637',
			'#cdd6f4',
			'#a6adc8',
			['#f38ba8', '#fab387', '#f9e2af', '#a6e3a1', '#89dceb', '#89b4fa', '#cba6f7'],
			'#cba6f7'
		),
		light: base(
			'#eff1f5',
			'#e6e9ef',
			'#4c4f69',
			'#6c6f85',
			['#d20f39', '#fe640b', '#df8e1d', '#40a02b', '#179299', '#1e66f5', '#8839ef'],
			'#8839ef'
		)
	},
	{
		id: 'gruvbox',
		label: 'Gruvbox',
		dark: base(
			'#282828',
			'#32302f',
			'#ebdbb2',
			'#a89984',
			['#fb4934', '#fe8019', '#fabd2f', '#b8bb26', '#8ec07c', '#83a598', '#d3869b'],
			'#fabd2f'
		),
		light: base(
			'#fbf1c7',
			'#f2e5bc',
			'#3c3836',
			'#7c6f64',
			['#9d0006', '#af3a03', '#b57614', '#79740e', '#427b58', '#076678', '#8f3f71'],
			'#af3a03'
		)
	},
	{
		id: 'nord',
		label: 'Nord',
		dark: base(
			'#2e3440',
			'#3b4252',
			'#d8dee9',
			'#9aa5b8',
			['#bf616a', '#d08770', '#ebcb8b', '#a3be8c', '#8fbcbb', '#81a1c1', '#b48ead'],
			'#88c0d0'
		),
		light: base(
			'#eceff4',
			'#e5e9f0',
			'#2e3440',
			'#4c566a',
			['#bf616a', '#d08770', '#b48a3c', '#6f8f58', '#5b9694', '#5e81ac', '#a07a9c'],
			'#5e81ac'
		)
	},
	{
		id: 'tokyo-night',
		label: 'Tokyo Night',
		dark: base('#1a1b26', '#24283b', '#c0caf5', '#9aa5ce', [
			'#f7768e',
			'#ff9e64',
			'#e0af68',
			'#9ece6a',
			'#7dcfff',
			'#7aa2f7',
			'#bb9af7'
		]),
		light: base('#e1e2e7', '#d5d6db', '#3760bf', '#6172b0', [
			'#f52a65',
			'#b15c00',
			'#8c6c3e',
			'#587539',
			'#007197',
			'#2e7de9',
			'#9854f1'
		])
	},
	{
		id: 'rose-pine',
		label: 'Rosé Pine',
		dark: base(
			'#191724',
			'#1f1d2e',
			'#e0def4',
			'#908caa',
			['#eb6f92', '#ea9a97', '#f6c177', '#9ccfa8', '#9ccfd8', '#31748f', '#c4a7e7'],
			'#c4a7e7'
		),
		light: base(
			'#faf4ed',
			'#fffaf3',
			'#575279',
			'#797593',
			['#b4637a', '#d7827e', '#ea9d34', '#5a9272', '#56949f', '#286983', '#907aa9'],
			'#907aa9'
		)
	},
	{
		id: 'everforest',
		label: 'Everforest',
		dark: base(
			'#2d353b',
			'#343f44',
			'#d3c6aa',
			'#9da9a0',
			['#e67e80', '#e69875', '#dbbc7f', '#a7c080', '#83c092', '#7fbbb3', '#d699b6'],
			'#a7c080'
		),
		light: base(
			'#fdf6e3',
			'#f4f0d9',
			'#5c6a72',
			'#829181',
			['#f85552', '#f57d26', '#dfa000', '#8da101', '#35a77c', '#3a94c5', '#df69ba'],
			'#8da101'
		)
	},
	{
		id: 'kanagawa',
		label: 'Kanagawa',
		dark: base('#1f1f28', '#2a2a37', '#dcd7ba', '#a6a69c', [
			'#e46876',
			'#ffa066',
			'#e6c384',
			'#98bb6c',
			'#7aa89f',
			'#7e9cd8',
			'#957fb8'
		]),
		light: base('#f2ecbc', '#e7dba0', '#545464', '#716e61', [
			'#c84053',
			'#cc6d00',
			'#77713f',
			'#6f894e',
			'#597b75',
			'#4d699b',
			'#624c83'
		])
	},
	{
		id: 'solarized',
		label: 'Solarized',
		dark: base('#002b36', '#073642', '#93a1a1', '#839496', [
			'#dc322f',
			'#cb4b16',
			'#b58900',
			'#859900',
			'#2aa198',
			'#268bd2',
			'#d33682'
		]),
		light: base('#fdf6e3', '#eee8d5', '#586e75', '#657b83', [
			'#dc322f',
			'#cb4b16',
			'#b58900',
			'#859900',
			'#2aa198',
			'#268bd2',
			'#d33682'
		])
	},
	{
		id: 'dracula',
		label: 'Dracula',
		dark: base(
			'#282a36',
			'#343746',
			'#f8f8f2',
			'#a4a9c4',
			['#ff5555', '#ffb86c', '#f1fa8c', '#50fa7b', '#8be9fd', '#bd93f9', '#ff79c6'],
			'#bd93f9'
		)
	}
];

/** Colours of the CSS schemes, for their swatches. */
const CSS_SWATCHES: Record<string, string[]> = {
	neutral: ['#0a0a0a', '#ffffff', '#737373', '#3987e5', '#d95926', '#199e70'],
	ccdash: ['#1c1c1a', '#f6f6f2', '#2a78d6', '#3987e5', '#d95926', '#199e70']
};

type Lch = { l: number; c: number; h: number };
type Rgb = [number, number, number];

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** `#rrggbb` as sRGB in 0–1. */
function parse(hex: string): Rgb {
	const n = parseInt(hex.slice(1), 16);
	return [(n >> 16) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

const format = (rgb: Rgb) =>
	`#${rgb
		.map((v) =>
			Math.round(clamp(v, 0, 1) * 255)
				.toString(16)
				.padStart(2, '0')
		)
		.join('')}`;

const toLinear = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const fromLinear = (v: number) => (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055);

/** https://bottosson.github.io/posts/oklab/ */
function oklab(hex: string): Rgb {
	const [r, g, b] = parse(hex).map(toLinear);
	const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
	const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
	const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
	return [
		0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
		1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
		0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
	];
}

/** Linear sRGB of an OKLab colour; outside 0–1 means out of gamut. */
function linearOf([L, a, b]: Rgb): Rgb {
	const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
	const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
	const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
	return [
		4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
		-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
		-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s
	];
}

function lch(hex: string): Lch {
	const [l, a, b] = oklab(hex);
	return { l, c: Math.hypot(a, b), h: ((Math.atan2(b, a) * 180) / Math.PI + 360) % 360 };
}

/** The colour as `#rrggbb`, with chroma given up until it fits sRGB. */
function fromLch({ l, c, h }: Lch): string {
	const rad = (h * Math.PI) / 180;
	for (let chroma = c; chroma > 0; chroma -= 0.004) {
		const rgb = linearOf([l, chroma * Math.cos(rad), chroma * Math.sin(rad)]);
		if (rgb.every((v) => v >= -0.0005 && v <= 1.0005)) return format(rgb.map(fromLinear) as Rgb);
	}
	return format(linearOf([l, 0, 0]).map(fromLinear) as Rgb);
}

/** `b` over `a` by `t`, in OKLab. */
export function mix(a: string, b: string, t: number): string {
	const [x, y] = [oklab(a), oklab(b)];
	const out = x.map((v, i) => v + (y[i] - v) * t) as Rgb;
	return format(linearOf(out).map(fromLinear) as Rgb);
}

function luminance(hex: string): number {
	const [r, g, b] = parse(hex).map(toLinear);
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio. */
export function contrast(a: string, b: string): number {
	const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
	return (x + 0.05) / (y + 0.05);
}

/** `colour`, its lightness moved away from `on` until it reads at `ratio`. */
export function legible(colour: string, on: string, ratio: number): string {
	if (contrast(colour, on) >= ratio) return colour;
	const start = lch(colour);
	const step = luminance(on) < 0.18 ? 0.01 : -0.01;
	for (let l = start.l; l >= 0 && l <= 1; l += step) {
		const next = fromLch({ ...start, l });
		if (contrast(next, on) >= ratio) return next;
	}
	return step > 0 ? '#ffffff' : '#000000';
}

/** The panel's CSS variables for a theme. */
export function tokens(b: Base, dark: boolean): Record<string, string> {
	const bg = b.bg;
	const fg = legible(b.fg, bg, 7);
	const card = b.surface ?? mix(bg, fg, 0.05);
	const raised = mix(bg, fg, 0.11);
	const muted = legible(b.muted ?? mix(fg, bg, 0.35), raised, 4.5);
	const sidebar = dark ? mix(bg, '#000000', 0.18) : mix(bg, fg, 0.04);
	const solid = legible(b.primary, card, 3);
	const onPrimary =
		contrast(solid, '#ffffff') >= contrast(solid, '#111111') ? '#ffffff' : '#111111';
	const primary = legible(solid, onPrimary, 4.5);
	const accent = mix(card, primary, 0.16);
	const line = mix(bg, fg, dark ? 0.15 : 0.13);
	const input = mix(bg, fg, dark ? 0.2 : 0.17);
	const ink = (colour: string) => legible(colour, card, 4.5);
	const [red, orange, yellow, green, cyan, blue, magenta] = [
		b.red,
		b.orange,
		b.yellow,
		b.green,
		b.cyan,
		b.blue,
		b.magenta
	].map(ink);
	const idle = mix(bg, fg, 0.3);
	const ramp = [0.2, 0.36, 0.52, 0.68, 0.84, 1].map((t) => mix(card, blue, t));
	return {
		'--background': bg,
		'--foreground': fg,
		'--card': card,
		'--card-foreground': fg,
		'--popover': card,
		'--popover-foreground': fg,
		'--primary': primary,
		'--primary-foreground': onPrimary,
		'--secondary': raised,
		'--secondary-foreground': fg,
		'--muted': raised,
		'--muted-foreground': muted,
		'--accent': accent,
		'--accent-foreground': fg,
		'--destructive': red,
		'--border': line,
		'--input': input,
		'--ring': primary,
		'--chart-1': blue,
		'--chart-2': green,
		'--chart-3': yellow,
		'--chart-4': magenta,
		'--chart-5': orange,
		'--sidebar': sidebar,
		'--sidebar-foreground': fg,
		'--sidebar-primary': primary,
		'--sidebar-primary-foreground': onPrimary,
		'--sidebar-accent': mix(sidebar, fg, 0.1),
		'--sidebar-accent-foreground': fg,
		'--sidebar-border': line,
		'--sidebar-ring': primary,
		'--working': blue,
		'--permission': orange,
		'--your-turn': green,
		'--idle': idle,
		'--good': green,
		'--warning': yellow,
		'--critical': red,
		'--kind-memory': blue,
		'--kind-issue': orange,
		'--kind-doc': green,
		'--kind-progress': muted,
		'--series-4': yellow,
		'--series-5': magenta,
		'--series-6': cyan,
		'--seq-1': ramp[0],
		'--seq-2': ramp[1],
		'--seq-3': ramp[2],
		'--seq-4': ramp[3],
		'--seq-5': ramp[4],
		'--seq-6': ramp[5],
		'--hl-keyword': magenta,
		'--hl-string': green,
		'--hl-number': orange,
		'--hl-title': blue,
		'--hl-type': yellow,
		'--hl-attr': cyan,
		'--hl-builtin': red,
		'--hl-comment': muted
	};
}

/** What each hue is for, its OKLCH angle, most telling first: a colour of
 * the palette serves one only, so states stay apart. */
const TARGETS: [keyof Base, number][] = [
	['red', 25],
	['green', 148],
	['blue', 258],
	['yellow', 92],
	['orange', 55],
	['magenta', 335],
	['cyan', 200]
];
const NEAR_DEG = 28;
/** Below this a colour reads as grey. */
const CHROMATIC = 0.045;
/** A state needs this much colour, however quiet the wallpaper. */
const STATE_CHROMA = 0.1;
/** How far a page may lie from black or white: no text reads on mid-grey. */
const DARK_PAGE_L = 0.32;
const LIGHT_PAGE_L = 0.88;

const hueGap = (a: number, b: number) => {
	const d = Math.abs(a - b) % 360;
	return d > 180 ? 360 - d : d;
};

/** How well a colour serves a hue, lower is better: its distance, less its strength. */
const fit = (v: Lch, h: number) => hueGap(v.h, h) / NEAR_DEG - v.c * 6;

/**
 * A machine's colours as a `Base`. Slots say nothing: pywal fills them from
 * the wallpaper in any order, and matugen's pywalfox file has text in slot 1.
 * So each hue takes the palette's nearest colour, and one the wallpaper does
 * not have is made at the palette's strength: a state stays recognisable.
 */
export function fromPalette(c: PaletteColours, dark: boolean): Base {
	const page = lch(c.background);
	const bg = fromLch({
		...page,
		l: dark ? Math.min(page.l, DARK_PAGE_L) : Math.max(page.l, LIGHT_PAGE_L)
	});
	const pool = [c.primary, c.secondary, c.tertiary, ...c.ansi]
		.filter((v): v is string => Boolean(v))
		.map((hex) => ({ hex, ...lch(hex) }))
		.filter((v) => v.c >= CHROMATIC && v.hex !== c.background && v.hex !== c.foreground);
	const strongest = pool.reduce((most, v) => Math.max(most, v.c), 0);
	const made = { l: dark ? 0.76 : 0.52, c: clamp(strongest, 0.09, 0.15) };
	const taken = new Set<string>();
	const hues = {} as Record<keyof Base, string>;
	for (const [name, h] of TARGETS) {
		// NOTE: strength counts: the nearest hue is often a dark shade of the wallpaper's grey
		const near = pool
			.filter((v) => !taken.has(v.hex) && hueGap(v.h, h) <= NEAR_DEG)
			.sort((p, q) => fit(p, h) - fit(q, h))[0];
		if (near) taken.add(near.hex);
		hues[name] = near
			? fromLch({ l: near.l, c: Math.max(near.c, STATE_CHROMA), h: near.h })
			: fromLch({ ...made, h });
	}
	const accent = pool.filter((v) => contrast(v.hex, bg) >= 3).sort((p, q) => q.c - p.c)[0]?.hex;
	return {
		bg,
		surface: c.surface ?? undefined,
		fg: c.foreground,
		muted: c.muted ?? undefined,
		primary: c.primary ?? accent ?? hues.blue,
		red: hues.red,
		orange: hues.orange,
		yellow: hues.yellow,
		green: hues.green,
		cyan: hues.cyan,
		blue: hues.blue,
		magenta: hues.magenta
	};
}

/** The mode a scheme shows when `dark` is wanted: its only one if it has one. */
const variant = <T>(modes: { dark?: T | null; light?: T | null }, dark: boolean) => {
	const [want, other] = dark ? [modes.dark, modes.light] : [modes.light, modes.dark];
	if (want) return { value: want, dark };
	return other ? { value: other, dark: !dark } : null;
};

/**
 * What a scheme means here: whether it is dark, and the variables to set
 * (`null`: the CSS schemes, and the wallpaper before any machine reported it).
 */
export function resolveScheme(
	id: string,
	dark: boolean,
	palette: Palette | null
): { dark: boolean; vars: Record<string, string> | null } {
	if (id === WALLPAPER) {
		const mode = palette && variant(palette, dark);
		if (!mode) return { dark, vars: null };
		return { dark: mode.dark, vars: tokens(fromPalette(mode.value, mode.dark), mode.dark) };
	}
	const scheme = SCHEMES.find((s) => s.id === id);
	const mode = scheme && variant(scheme, dark);
	if (!mode) return { dark, vars: null };
	return { dark: mode.dark, vars: tokens(mode.value, mode.dark) };
}

/** A scheme's face in a picker, darkest mode first. */
export function swatch(scheme: Scheme): string[] {
	if (scheme.css) return CSS_SWATCHES[scheme.id] ?? [];
	const b = scheme.dark ?? scheme.light;
	return b ? [b.bg, b.surface ?? b.bg, b.primary, b.blue, b.orange, b.green] : [];
}

export function paletteSwatch(palette: Palette): string[] {
	const c = palette.dark ?? palette.light;
	if (!c) return [];
	const b = fromPalette(c, Boolean(palette.dark));
	return [b.bg, b.surface ?? b.bg, b.primary, b.blue, b.orange, b.green];
}

export const isScheme = (id: string | null): id is string =>
	id === WALLPAPER || SCHEMES.some((s) => s.id === id);
