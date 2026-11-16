// Dictation: the field's microphone. The browser records, the recording
// goes to the hub as an attachment, a machine with a `[transcribe]
// command` puts it into words, and the words land in the field to read
// over before they are sent. The recording is not kept.

import { browser } from '$app/env';
import { Unauthorized, postJson } from './api/client';
import { upload } from './attachments.svelte';
import { live } from './live/store.svelte';
import { prefs } from './prefs.svelte';

/** A recording stops by itself after this long. */
export const MAX_SECONDS = 600;
/** What the recorder may write, the first this browser can. */
const TYPES = [
	['audio/webm;codecs=opus', 'webm'],
	['audio/mp4', 'm4a'],
	['audio/ogg;codecs=opus', 'ogg']
] as const;

/**
 * The machine that puts a recording into words: the one Settings names,
 * else `preferred` (the session's host: its files are there anyway), else
 * the machine this device sits on, else the first connected one that can.
 *
 * @param preferred - The session's or the start sheet's machine.
 * @returns Its name, or null when no connected machine transcribes.
 */
export function dictationHostFor(preferred?: string | null): string | null {
	if (!live.hub?.features.attachments) return null;
	const can = (name: string | null | undefined) =>
		Boolean(name) &&
		live.hosts.some((h) => h.name === name && h.connected && h.features.transcribe);
	if (can(prefs.dictationHost)) return prefs.dictationHost;
	if (can(preferred)) return preferred ?? null;
	if (can(live.here)) return live.here;
	return live.hosts.find((h) => h.connected && h.features.transcribe)?.name ?? null;
}

/** Whether this browser can record at all (a secure page with a recorder). */
export const canRecord = () =>
	browser && Boolean(navigator.mediaDevices?.getUserMedia) && typeof MediaRecorder !== 'undefined';

export type DictationState = 'idle' | 'asking' | 'recording' | 'working';

/** One field's dictation: record, then words. */
export class Dictation {
	state = $state<DictationState>('idle');
	/** Seconds recorded so far. */
	seconds = $state(0);
	/** How loud the microphone hears, 0 to 1, while recording. */
	level = $state(0);
	error = $state('');
	/** The machine working on the words. */
	host = $state('');
	#recorder: MediaRecorder | null = null;
	#stream: MediaStream | null = null;
	#audio: AudioContext | null = null;
	#pieces: Blob[] = [];
	#timer: ReturnType<typeof setInterval> | null = null;
	#frame = 0;
	#cancelled = false;
	#abort: AbortController | null = null;

	/**
	 * @param session - The session the field writes to, noted on the upload.
	 * @param preferred - The field's machine now (the session's, the start sheet's pick).
	 * @param onwords - Takes the words once a machine made them.
	 */
	constructor(
		readonly session: string | undefined,
		readonly preferred: () => string | null | undefined,
		readonly onwords: (text: string) => void
	) {}

	get busy(): boolean {
		return this.state !== 'idle';
	}

	/** Asks for the microphone and starts recording. */
	async start() {
		if (this.busy) return;
		this.error = '';
		if (!canRecord()) {
			this.error = isSecureContext
				? 'This browser cannot record.'
				: 'Recording needs the panel opened over https.';
			return;
		}
		this.state = 'asking';
		try {
			this.#stream = await navigator.mediaDevices.getUserMedia({ audio: true });
		} catch (e) {
			this.state = 'idle';
			this.error =
				(e as Error).name === 'NotAllowedError'
					? 'The microphone is not allowed for this page: allow it in the browser and try again.'
					: `No microphone: ${(e as Error).message}`;
			return;
		}
		const type = TYPES.find(([mime]) => MediaRecorder.isTypeSupported(mime));
		this.#recorder = new MediaRecorder(this.#stream, type ? { mimeType: type[0] } : undefined);
		this.#pieces = [];
		this.#cancelled = false;
		this.#recorder.ondataavailable = (e) => {
			if (e.data.size) this.#pieces.push(e.data);
		};
		this.#recorder.onstop = () => this.#recorded(type?.[1] ?? 'webm');
		this.#recorder.start(1000);
		this.seconds = 0;
		this.state = 'recording';
		const started = Date.now();
		this.#timer = setInterval(() => {
			this.seconds = Math.floor((Date.now() - started) / 1000);
			if (this.seconds >= MAX_SECONDS) this.done();
		}, 250);
		this.#meter();
	}

	/** Shows the level, so a muted or wrong microphone is seen at once. */
	#meter() {
		if (!this.#stream) return;
		try {
			this.#audio = new AudioContext();
			const analyser = this.#audio.createAnalyser();
			analyser.fftSize = 512;
			this.#audio.createMediaStreamSource(this.#stream).connect(analyser);
			const samples = new Uint8Array(analyser.fftSize);
			const tick = () => {
				analyser.getByteTimeDomainData(samples);
				let peak = 0;
				for (const s of samples) peak = Math.max(peak, Math.abs(s - 128) / 128);
				this.level = Math.max(peak, this.level * 0.85);
				this.#frame = requestAnimationFrame(tick);
			};
			tick();
		} catch {
			// NOTE: no meter is no reason not to record
		}
	}

	#release() {
		if (this.#timer) clearInterval(this.#timer);
		this.#timer = null;
		cancelAnimationFrame(this.#frame);
		this.#audio?.close().catch(() => {});
		this.#audio = null;
		for (const track of this.#stream?.getTracks() ?? []) track.stop();
		this.#stream = null;
		this.level = 0;
	}

	/** Ends the recording and has it put into words. */
	done() {
		if (this.state === 'recording') this.#recorder?.stop();
	}

	/** Throws the recording away, or stops waiting for its words. */
	cancel() {
		this.#cancelled = true;
		this.#abort?.abort();
		if (this.state === 'recording') this.#recorder?.stop();
		else this.#release();
		this.state = 'idle';
	}

	async #recorded(extension: string) {
		this.#release();
		const blob = new Blob(this.#pieces, { type: this.#recorder?.mimeType || 'audio/webm' });
		this.#pieces = [];
		this.#recorder = null;
		if (this.#cancelled) return;
		if (!blob.size) {
			this.state = 'idle';
			this.error = 'Nothing was recorded.';
			return;
		}
		const host = dictationHostFor(this.preferred());
		if (!host) {
			this.state = 'idle';
			this.error = 'No connected machine puts recordings into words right now.';
			return;
		}
		this.host = host;
		this.state = 'working';
		this.#abort = new AbortController();
		try {
			const file = new File([blob], `dictation.${extension}`, { type: blob.type });
			const recording = await upload(file, { session: this.session, signal: this.#abort.signal });
			const { text } = await postJson<{ text: string }>(`attachments/${recording.id}/transcribe`, {
				host
			});
			if (!this.#cancelled) this.onwords(text);
		} catch (e) {
			if (this.#cancelled || (e as Error).name === 'AbortError') return;
			if (e instanceof Unauthorized) live.needToken = true;
			else this.error = (e as Error).message;
		} finally {
			this.#abort = null;
			if (!this.#cancelled) this.state = 'idle';
		}
	}

	/** The field goes away: the microphone is let go. */
	destroy() {
		this.cancel();
	}
}
