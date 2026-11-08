// One live store for the whole panel: the snapshot and the SSE stream
// (sessions, hosts, limits, note changes), reconnecting with backoff and
// restarting a stream that went quiet. Pages derive their views from it.

import { browser } from '$app/env';
import { SvelteMap } from 'svelte/reactivity';
import { Unauthorized, api, getJson, token } from '../api/client';
import type {
	Draft,
	HostFeatures,
	HostStatus,
	HubStatus,
	Launch,
	RateLimits,
	Session,
	Snapshot,
	Update
} from '../api/types';

const STALE_STREAM_MS = 45_000;

const NO_FEATURES: HostFeatures = {
	edit: false,
	attach: false,
	launch: false,
	channel: false,
	sync: false,
	workspace: false
};

export type ConnKind = 'connecting' | 'live' | 'reconnecting' | 'offline';

class Live {
	host = $state('');
	/** Hub clock minus ours. */
	skew = $state(0);
	sessions = new SvelteMap<string, Session>();
	/** Launches this panel saw on the stream, by id. */
	launches = new SvelteMap<string, Launch>();
	/** The prompts being written, on every device, by draft key. */
	drafts = new SvelteMap<string, Draft>();
	/** The machine this device sits on, as the hub tells it from the
	 * address (a collector links from the same one); null when none does. */
	here = $state<string | null>(null);
	#hereAt = 0;
	rateLimits = $state<RateLimits | null>(null);
	workspaces = $state<string[]>([]);
	hosts = $state<HostStatus[]>([]);
	/** The hub's own machine. */
	hub = $state<HubStatus | null>(null);
	conn = $state<{ kind: ConnKind; text: string }>({ kind: 'connecting', text: 'connecting' });
	/** The token was rejected or is missing; the shell asks for one. */
	needToken = $state(false);
	/** Bumped when a workspace's notes change. */
	notes = $state<{ workspace: string; n: number }>({ workspace: '', n: 0 });
	/** Ticks every second, hub time. */
	now = $state(Date.now());
	/** The snapshot arrived at least once. */
	ready = $state(false);

	#abort: AbortController | null = null;
	#streaming = false;
	#last = 0;
	#timers: ReturnType<typeof setInterval>[] = [];

	get list(): Session[] {
		return [...this.sessions.values()];
	}

	get multiHost(): boolean {
		return this.hosts.length > 1;
	}

	connected(host: string): boolean {
		return this.hosts.some((h) => h.name === host && h.connected);
	}

	/** What the host's collector does besides reporting; nothing for a host
	 * not heard from. */
	features(host: string): HostFeatures {
		return this.hosts.find((h) => h.name === host)?.features ?? NO_FEATURES;
	}

	/** Whether any machine's collector does `feature`. */
	anyHost(feature: keyof HostFeatures): boolean {
		return this.hosts.some((h) => h.features[feature]);
	}

	hostLabel(host: string): string {
		const status = this.hosts.find((x) => x.name === host);
		return status && !status.connected ? `${host} (offline)` : host;
	}

	start() {
		if (!browser || this.#timers.length) return;
		this.#timers.push(setInterval(() => (this.now = Date.now() + this.skew), 1000));
		this.#timers.push(setInterval(() => this.reconnectIfStale(), 10_000));
		document.addEventListener('visibilitychange', () => {
			if (document.visibilityState === 'visible') this.reconnectIfStale();
		});
		if (!token()) {
			this.needToken = true;
			this.setConn('offline', 'token needed');
			return;
		}
		this.run();
	}

	/** Starts streaming (again) once a token is there. */
	run() {
		if (this.#streaming) return;
		this.needToken = false;
		this.#streaming = true;
		this.loop().finally(() => (this.#streaming = false));
	}

	setConn(kind: ConnKind, text: string = kind) {
		this.conn = { kind, text };
	}

	/** After this device's token was revoked: the stream ends, the shell asks for a token. */
	signOut() {
		this.needToken = true;
		this.setConn('offline', 'token needed');
		this.#abort?.abort();
	}

	reconnectIfStale() {
		if (this.#abort && Date.now() - this.#last > STALE_STREAM_MS) this.#abort.abort();
	}

	private async loop() {
		let delay = 1000;
		for (;;) {
			this.#abort = new AbortController();
			try {
				this.setConn('connecting');
				// NOTE: a connect in flight is not a stale stream: the watchdog would abort it at its next tick
				this.#last = Date.now();
				// NOTE: the HTTP cache may hold a second GET of this URL until the first ends, and a stream never does
				const res = await api('stream', { signal: this.#abort.signal, cache: 'no-store' });
				this.setConn('live');
				delay = 1000;
				this.#last = Date.now();
				const reader = res.body!.pipeThrough(new TextDecoderStream()).getReader();
				let buffer = '';
				for (;;) {
					const { value, done } = await reader.read();
					if (done) break;
					this.#last = Date.now();
					buffer += value;
					let end;
					while ((end = buffer.indexOf('\n\n')) >= 0) {
						this.handle(buffer.slice(0, end));
						buffer = buffer.slice(end + 2);
					}
				}
			} catch (e) {
				if (e instanceof Unauthorized || this.needToken) {
					this.needToken = true;
					this.setConn('offline', 'token needed');
					return;
				}
			}
			this.setConn('reconnecting');
			await new Promise((resolve) => setTimeout(resolve, delay));
			delay = Math.min(delay * 2, 30_000);
		}
	}

	private handle(raw: string) {
		let event = 'message';
		let data = '';
		for (const line of raw.split('\n')) {
			if (line.startsWith('event:')) event = line.slice(6).trim();
			else if (line.startsWith('data:')) data += line.slice(5).trimStart();
		}
		if (event === 'lagged') return void this.refreshSnapshot();
		if (!data) return;
		const payload = JSON.parse(data);
		if (event === 'snapshot') this.applySnapshot(payload);
		else if (event === 'update') this.applyUpdate(payload);
	}

	async refreshSnapshot() {
		try {
			this.applySnapshot(await getJson<Snapshot>('snapshot'));
		} catch (e) {
			if (e instanceof Unauthorized) this.needToken = true;
		}
	}

	applySnapshot(snapshot: Snapshot) {
		this.host = snapshot.host;
		this.skew = snapshot.now - Date.now();
		this.now = Date.now() + this.skew;
		const ids = new Set(snapshot.sessions.map((s) => s.id));
		for (const id of [...this.sessions.keys()]) if (!ids.has(id)) this.sessions.delete(id);
		for (const s of snapshot.sessions) this.sessions.set(s.id, s);
		this.rateLimits = snapshot.rate_limits;
		this.workspaces = snapshot.workspaces ?? [];
		this.hosts = snapshot.hosts ?? [];
		this.hub = snapshot.hub ?? null;
		const keys = new Set((snapshot.drafts ?? []).map((d) => d.key));
		for (const key of [...this.drafts.keys()]) if (!keys.has(key)) this.drafts.delete(key);
		for (const d of snapshot.drafts ?? []) this.drafts.set(d.key, d);
		this.ready = true;
		this.locate();
	}

	/** Asks the hub which machine this device sits on, at most once a
	 * minute (a snapshot comes with every reconnect). */
	locate() {
		const now = Date.now();
		if (now - this.#hereAt < 60_000) return;
		this.#hereAt = now;
		getJson<{ host: string | null }>('here')
			.then(({ host }) => (this.here = host))
			.catch(() => {});
	}

	applyUpdate(update: Update) {
		switch (update.type) {
			case 'session':
				this.sessions.set(update.session.id, update.session);
				break;
			case 'removed':
				this.sessions.delete(update.id);
				break;
			case 'rate_limits':
				this.rateLimits = update.rate_limits;
				break;
			case 'hosts':
				this.hosts = update.hosts;
				break;
			case 'hub':
				this.hub = update.hub;
				break;
			case 'notes':
				this.notes = { workspace: update.workspace, n: this.notes.n + 1 };
				break;
			case 'launch':
				this.launches.set(update.launch.id, update.launch);
				break;
			case 'draft':
				this.putDraft(update.draft);
				break;
		}
	}

	/** Puts a draft the stream or the API returned into the store; an empty
	 * one with no editor is gone. */
	putDraft(draft: Draft) {
		if (!draft.text && !draft.editing && !draft.notice) this.drafts.delete(draft.key);
		else this.drafts.set(draft.key, draft);
	}

	/** Puts a host status the API returned into the store. */
	putHost(status: HostStatus) {
		const i = this.hosts.findIndex((h) => h.name === status.name);
		if (i < 0) this.hosts = [...this.hosts, status];
		else this.hosts[i] = status;
	}

	/** Puts a session the API returned into the store. */
	put(session: Session) {
		this.sessions.set(session.id, session);
	}
}

export const live = new Live();
