// Syntax colours for code blocks. highlight.js runs here rather than on the
// hub so the colours follow the theme and the Pi renders nothing; the core
// and each language are chunks of their own, fetched on first use.

import type { HLJSApi, LanguageFn } from 'highlight.js';

/** Past this a block stays plain: colouring it would hold up the page. */
const MAX_CHARS = 100_000;

type Loader = () => Promise<{ default: LanguageFn }>;

const LANGUAGES: Record<string, Loader> = {
	bash: () => import('highlight.js/lib/languages/bash'),
	c: () => import('highlight.js/lib/languages/c'),
	cpp: () => import('highlight.js/lib/languages/cpp'),
	csharp: () => import('highlight.js/lib/languages/csharp'),
	css: () => import('highlight.js/lib/languages/css'),
	diff: () => import('highlight.js/lib/languages/diff'),
	dockerfile: () => import('highlight.js/lib/languages/dockerfile'),
	go: () => import('highlight.js/lib/languages/go'),
	haskell: () => import('highlight.js/lib/languages/haskell'),
	ini: () => import('highlight.js/lib/languages/ini'),
	java: () => import('highlight.js/lib/languages/java'),
	javascript: () => import('highlight.js/lib/languages/javascript'),
	json: () => import('highlight.js/lib/languages/json'),
	lua: () => import('highlight.js/lib/languages/lua'),
	makefile: () => import('highlight.js/lib/languages/makefile'),
	markdown: () => import('highlight.js/lib/languages/markdown'),
	nginx: () => import('highlight.js/lib/languages/nginx'),
	nix: () => import('highlight.js/lib/languages/nix'),
	php: () => import('highlight.js/lib/languages/php'),
	python: () => import('highlight.js/lib/languages/python'),
	ruby: () => import('highlight.js/lib/languages/ruby'),
	rust: () => import('highlight.js/lib/languages/rust'),
	scss: () => import('highlight.js/lib/languages/scss'),
	shell: () => import('highlight.js/lib/languages/shell'),
	sql: () => import('highlight.js/lib/languages/sql'),
	typescript: () => import('highlight.js/lib/languages/typescript'),
	xml: () => import('highlight.js/lib/languages/xml'),
	yaml: () => import('highlight.js/lib/languages/yaml')
};

const ALIASES: Record<string, string> = {
	'c++': 'cpp',
	'c#': 'csharp',
	cs: 'csharp',
	console: 'shell',
	docker: 'dockerfile',
	golang: 'go',
	h: 'c',
	hpp: 'cpp',
	hs: 'haskell',
	html: 'xml',
	js: 'javascript',
	jsonc: 'json',
	jsx: 'javascript',
	make: 'makefile',
	md: 'markdown',
	py: 'python',
	rb: 'ruby',
	rs: 'rust',
	sh: 'bash',
	svelte: 'xml',
	svg: 'xml',
	terminal: 'shell',
	toml: 'ini',
	ts: 'typescript',
	tsx: 'typescript',
	vue: 'xml',
	yml: 'yaml',
	zsh: 'bash'
};

let core: Promise<HLJSApi> | null = null;
const loaded: Record<string, Promise<void>> = {};

const known = (name: string) => {
	const key = name.toLowerCase();
	return LANGUAGES[key] ? key : ALIASES[key];
};

/** The code as highlight.js's escaped markup, or null to show it plain: no
 * or an unknown language, too long, or the chunks could not be fetched. */
export async function highlight(code: string, name: string | null): Promise<string | null> {
	const language = name ? known(name) : undefined;
	if (!language || code.length > MAX_CHARS) return null;
	try {
		core ??= import('highlight.js/lib/core').then((m) => m.default);
		const hljs = await core;
		loaded[language] ??= LANGUAGES[language]().then((m) => {
			hljs.registerLanguage(language, m.default);
		});
		await loaded[language];
		return hljs.highlight(code, { language, ignoreIllegals: true }).value;
	} catch {
		// NOTE: offline with a stale worker: forget it, so the next block tries again
		core = null;
		delete loaded[language];
		return null;
	}
}
