// Preferences that stay on the device: theme, colour scheme and the machine
// whose wallpaper it follows, the sidebar, the board grid, composer drafts,
// quick replies and templates, the last host, model and permission mode a
// session was started with.

import { browser } from '$app/env';
import type { HostStatus, Palette } from './api/types';
import { WALLPAPER, isScheme, resolveScheme } from './theme';

export type Theme = 'auto' | 'light' | 'dark';

const THEME_KEY = 'ccdash.theme';
const SCHEME_KEY = 'ccdash.scheme';
const WALL_HOST_KEY = 'ccdash.wallhost';
const PALETTE_KEY = 'ccdash.palette';
const SIDEBAR_KEY = 'ccdash.sidebar';
const GRID_KEY = 'ccdash.grid';
const QUICK_KEY = 'ccdash.quick';
const TEMPLATES_KEY = 'ccdash.templates';
const DRAFT_KEY = 'ccdash.draft.';
const START_KEY = 'ccdash.start';

export const DEFAULT_QUICK = [
	'Continue',
	'Yes',
	'No',
	'Explain what you did',
	'Run the tests and report'
];

export type Template = { name: string; text: string };

/** What the start sheet picked last; empty strings are the host's defaults. */
export type StartChoice = { host: string; model: string; mode: string };

/** The wallpaper colours in use: kept, so the theme is there before the
 * hub answers and when no machine reports. */
export type Followed = { host: string; palette: Palette };

function load<T>(key: string, fallback: T): T {
	if (!browser) return fallback;
	try {
		const raw = localStorage.getItem(key);
		return raw ? (JSON.parse(raw) as T) : fallback;
	} catch {
		return fallback;
	}
}

const schemeOf = (id: string | null) => (isScheme(id) ? id : 'neutral');

class Prefs {
	#applied: string[] = [];
	theme = $state<Theme>(browser ? ((localStorage.getItem(THEME_KEY) as Theme) ?? 'auto') : 'auto');
	scheme = $state<string>(schemeOf(browser ? localStorage.getItem(SCHEME_KEY) : null));
	/** The machine the wallpaper scheme follows; empty, the first that reports. */
	wallHost = $state<string>(browser ? (localStorage.getItem(WALL_HOST_KEY) ?? '') : '');
	followed = $state.raw<Followed | null>(load<Followed | null>(PALETTE_KEY, null));
	/** The full sidebar on wide screens; off, a rail of icons (a narrow
	 * vertical monitor). */
	sidebar = $state<boolean>(!browser || localStorage.getItem(SIDEBAR_KEY) !== 'off');
	/** A light grid behind the project pages. */
	grid = $state<boolean>(browser && localStorage.getItem(GRID_KEY) === 'on');
	quick = $state<string[]>(load(QUICK_KEY, DEFAULT_QUICK));
	templates = $state<Template[]>(load(TEMPLATES_KEY, []));
	start = $state<StartChoice>(load(START_KEY, { host: '', model: '', mode: '' }));

	setTheme(theme: Theme) {
		this.theme = theme;
		localStorage.setItem(THEME_KEY, theme);
		this.applyTheme();
	}

	applyTheme() {
		if (!browser) return;
		const wanted =
			this.theme === 'dark' ||
			(this.theme === 'auto' && matchMedia('(prefers-color-scheme: dark)').matches);
		const { dark, vars } = resolveScheme(this.scheme, wanted, this.followed?.palette ?? null);
		const root = document.documentElement;
		root.classList.toggle('dark', dark);
		root.style.colorScheme = dark ? 'dark' : 'light';
		root.dataset.scheme = this.scheme;
		for (const name of this.#applied) if (!vars?.[name]) root.style.removeProperty(name);
		for (const [name, value] of Object.entries(vars ?? {})) root.style.setProperty(name, value);
		this.#applied = Object.keys(vars ?? {});
		for (const meta of document.querySelectorAll<HTMLMetaElement>('meta[name=theme-color]')) {
			meta.dataset.stock ??= meta.content;
			meta.content = vars?.['--background'] ?? meta.dataset.stock;
		}
	}

	setScheme(scheme: string) {
		this.scheme = scheme;
		localStorage.setItem(SCHEME_KEY, scheme);
		this.applyTheme();
	}

	setWallHost(host: string, hosts: HostStatus[]) {
		this.wallHost = host;
		localStorage.setItem(WALL_HOST_KEY, host);
		this.takePalettes(hosts);
	}

	/** Takes what the followed machine reports, as the wallpaper changes. A
	 * machine that went quiet keeps its last colours. */
	takePalettes(hosts: HostStatus[]) {
		const host = this.wallHost
			? hosts.find((h) => h.name === this.wallHost && h.palette)
			: hosts.find((h) => h.palette);
		if (!host?.palette) return;
		const next = { host: host.name, palette: host.palette };
		if (JSON.stringify(next) === JSON.stringify(this.followed)) return;
		this.followed = next;
		localStorage.setItem(PALETTE_KEY, JSON.stringify(next));
		if (this.scheme === WALLPAPER) this.applyTheme();
	}

	setSidebar(on: boolean) {
		this.sidebar = on;
		localStorage.setItem(SIDEBAR_KEY, on ? 'on' : 'off');
	}

	setGrid(on: boolean) {
		this.grid = on;
		localStorage.setItem(GRID_KEY, on ? 'on' : 'off');
	}

	setQuick(list: string[]) {
		this.quick = list.map((s) => s.trim()).filter(Boolean);
		localStorage.setItem(QUICK_KEY, JSON.stringify(this.quick));
	}

	setTemplates(list: Template[]) {
		this.templates = list.filter((t) => t.name.trim() && t.text.trim());
		localStorage.setItem(TEMPLATES_KEY, JSON.stringify(this.templates));
	}

	setStart(choice: StartChoice) {
		this.start = choice;
		localStorage.setItem(START_KEY, JSON.stringify(choice));
	}

	draft(session: string): string {
		return browser ? (localStorage.getItem(DRAFT_KEY + session) ?? '') : '';
	}

	setDraft(session: string, text: string) {
		if (text) localStorage.setItem(DRAFT_KEY + session, text);
		else localStorage.removeItem(DRAFT_KEY + session);
	}
}

export const prefs = new Prefs();
