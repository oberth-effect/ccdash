// The viewport sweep: opens the panel's main pages on a running demo hub
// (`ccdash demo`) at the sizes ccdash is used at, checks each for what
// breaks first on a small screen, and writes a screenshot of every page.
//
//   node web/scripts/viewports.mjs [--base http://127.0.0.1:7840] [--out DIR] [--only phone-small,laptop]
//
// Needs `PLAYWRIGHT_CORE` (the playwright-core package directory) and
// `PLAYWRIGHT_BROWSERS_PATH`; docs/development.md has the Nix lines.
// Exits 1 when a check fails.

import { mkdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { parseArgs } from 'node:util';

/** Width × height in CSS pixels, the device pixel ratio, touch. */
const VIEWPORTS = {
	'phone-small': [360, 740, 3, true],
	phone: [412, 915, 2.6, true],
	'phone-landscape': [740, 360, 3, true],
	tablet: [800, 1280, 2, true],
	laptop: [1920, 1080, 1, false],
	'desktop-4k': [2560, 1440, 1.5, false]
};

/** The demo's sessions on the owner's turn and at work, which adds Stop (`src/demo/script.rs`). */
const SESSION = '9c6e2a0b-4d8f-4b1c-d5e9-7a3f0c6b4e8a';
const WORKING = '7a4c0e8f-2b6d-4f9a-b3c7-5e1d8a4f2c6e';
const PAGES = [
	['now', '/'],
	['sessions', '/sessions'],
	['chat', `/sessions/${SESSION}`],
	['chat-working', `/sessions/${WORKING}`],
	['projects', '/projects'],
	['machines', '/machines'],
	['files', '/files'],
	['settings', '/settings/appearance']
];
/** The chat must keep at least this share of a session page's height. */
const MIN_CHAT_SHARE = 0.4;
const MIN_CHAT_SHARE_LANDSCAPE = 0.15;
/** The composer's buttons are one row: its bar is at most this tall, in root em. */
const MAX_BAR_REM = 3;

const { values: args } = parseArgs({
	options: {
		base: { type: 'string', default: 'http://127.0.0.1:7840' },
		out: { type: 'string', default: 'viewports' },
		only: { type: 'string' },
		token: { type: 'string', default: 'demo' }
	}
});

if (!process.env.PLAYWRIGHT_CORE) {
	console.error('PLAYWRIGHT_CORE is not set: see docs/development.md, "Viewports".');
	process.exit(2);
}
const { chromium } = createRequire(import.meta.url)(process.env.PLAYWRIGHT_CORE);

const wanted = args.only?.split(',') ?? Object.keys(VIEWPORTS);
const unknown = wanted.filter((name) => !VIEWPORTS[name]);
if (unknown.length) {
	console.error(
		`Unknown viewport ${unknown.join(', ')}; there are ${Object.keys(VIEWPORTS).join(', ')}.`
	);
	process.exit(2);
}
mkdirSync(args.out, { recursive: true });

/** What is wrong with the page as it stands; empty when nothing is. */
async function problems(page, name, landscape) {
	const found = await page.evaluate(() => {
		const out = [];
		const root = document.documentElement;
		if (root.scrollWidth > innerWidth)
			out.push(`the page is ${root.scrollWidth - innerWidth}px wider than the screen`);
		for (const el of document.querySelectorAll(
			'main button, main a, header button, footer button'
		)) {
			const box = el.getBoundingClientRect();
			if (!box.width || !box.height) continue;
			// NOTE: inside a scrolling strip (quick replies, a table) is not off the screen
			let strip = false;
			for (let up = el.parentElement; up && !strip; up = up.parentElement)
				strip = /auto|scroll/.test(getComputedStyle(up).overflowX);
			if (!strip && box.right > innerWidth + 1)
				out.push(
					`"${(el.getAttribute('aria-label') ?? el.textContent).trim().slice(0, 30)}" runs off the right edge`
				);
		}
		return out;
	});
	if (!name.startsWith('chat')) return found;
	const session = await page.evaluate(() => {
		const footer = document.querySelector('footer');
		const tabs = document.querySelector('[role=tablist]');
		if (!footer || !tabs) return null;
		const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
		return {
			bar: (footer.querySelector('[data-composer-bar]')?.getBoundingClientRect().height ?? 0) / rem,
			chat: (footer.getBoundingClientRect().top - tabs.getBoundingClientRect().bottom) / innerHeight
		};
	});
	if (!session) return [...found, 'the session page has no composer'];
	if (!session.bar || session.bar > MAX_BAR_REM)
		found.push(`the composer's buttons take ${session.bar.toFixed(1)} rem: more than one row`);
	const least = landscape ? MIN_CHAT_SHARE_LANDSCAPE : MIN_CHAT_SHARE;
	if (session.chat < least)
		found.push(
			`the chat has ${Math.round(session.chat * 100)}% of the height, under ${least * 100}%`
		);
	return found;
}

const browser = await chromium.launch();
let failed = 0;
try {
	for (const name of wanted) {
		const [width, height, deviceScaleFactor, touch] = VIEWPORTS[name];
		const context = await browser.newContext({
			viewport: { width, height },
			screen: { width, height },
			deviceScaleFactor,
			isMobile: touch,
			hasTouch: touch
		});
		const page = await context.newPage();
		const errors = [];
		page.on('pageerror', (e) => errors.push(String(e)));
		try {
			await page.goto(`${args.base}/#token=${args.token}`);
			await page.waitForSelector('main');
		} catch {
			console.error(
				`No panel answers at ${args.base}: start \`ccdash demo\` first, or pass --base.`
			);
			await browser.close();
			process.exit(2);
		}
		for (const [label, path] of PAGES) {
			// NOTE: a goto that only changes the hash does not load; go through a blank page
			await page.goto('about:blank');
			await page.goto(args.base + path);
			await page.waitForSelector(label.startsWith('chat') ? '.cm-content' : 'main');
			await page.waitForTimeout(1500);
			await page.screenshot({ path: join(args.out, `${name}-${label}.png`) });
			const found = [
				...(await problems(page, label, width > height && touch)),
				...errors.splice(0)
			];
			failed += found.length;
			console.log(`${found.length ? 'FAIL' : 'ok  '} ${name} ${width}×${height} ${path}`);
			for (const line of found) console.log(`       ${line}`);
		}
		await context.close();
	}
} finally {
	await browser.close();
}
console.log(
	failed ? `${failed} problems; screenshots in ${args.out}` : `All fine; screenshots in ${args.out}`
);
process.exit(failed ? 1 : 0);
