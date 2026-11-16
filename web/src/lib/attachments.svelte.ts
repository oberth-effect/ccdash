// Attachments in the panel: files uploaded to the hub for a prompt. A
// prompt carries one as a marker line (`[ccdash-attachment <id>]`); the
// session machine's collector puts the local path in its place before
// Claude reads it (`src/attachments.rs`). Here: the markers and the lines
// they become (to show the files in the chat instead), photos made
// smaller before the upload, the upload itself with its progress, and the
// tray of files a prompt being written holds.

import { browser } from '$app/env';
import { ApiError, Unauthorized, del, getJson, query, token } from './api/client';
import type { Attachment } from './api/types';
import { prefs } from './prefs.svelte';

const ID = '[0-9a-f]{24}';
const MARKER = new RegExp(`\\[ccdash-attachment (${ID})\\]`, 'g');
/** NOTE: the collector's line names the file under `…/<id>/<name>` (`attachments::delivered`). */
const DELIVERED = new RegExp(`\\[Attached [^\\]\\n]*?: [^\\]\\n]*/(${ID})/[^\\]\\n/]+\\]`, 'g');
const UNDELIVERED = new RegExp(
	`\\[An attachment \\((${ID})\\) did not reach this machine: ([^\\]\\n]*?)\\. Tell the user it is missing\\.\\]`,
	'g'
);

export const marker = (id: string) => `[ccdash-attachment ${id}]`;

/** A prompt's attachments apart from its words. */
export type Parsed = {
	/** The text without the attachment lines. */
	text: string;
	ids: string[];
	/** Why an attachment did not reach the session's machine, by id. */
	failed: Record<string, string>;
};

/**
 * Takes the attachment lines out of a prompt, whether still markers (as
 * sent) or already the collector's lines (as Claude read them).
 *
 * @param text - The prompt or message.
 * @returns The words, the attachment ids in order, and the ones that failed.
 */
export function parse(text: string): Parsed {
	const ids: string[] = [];
	const failed: Record<string, string> = {};
	const note = (id: string) => {
		if (!ids.includes(id)) ids.push(id);
		return '';
	};
	const rest = text
		.replace(UNDELIVERED, (_, id: string, why: string) => {
			failed[id] = why;
			return note(id);
		})
		.replace(DELIVERED, (_, id: string) => note(id))
		.replace(MARKER, (_, id: string) => note(id));
	return { text: ids.length ? rest.replace(/\n{3,}/g, '\n\n').trim() : text, ids, failed };
}

/** The prompt as sent: the words, then a marker line per attachment. */
export function compose(text: string, ids: string[]): string {
	const body = text.trim();
	if (!ids.length) return body;
	return [body, ids.map(marker).join('\n')].filter(Boolean).join('\n\n');
}

const metas: Record<string, Promise<Attachment | null>> = {};

/**
 * An attachment's row with a link to show it through, cached for the
 * page's life.
 *
 * @param id - The attachment.
 * @returns The row, or null when the hub no longer has it.
 * @throws Unauthorized when the device's token is refused.
 */
export function attachmentOf(id: string): Promise<Attachment | null> {
	metas[id] ??= getJson<{ attachment: Attachment }>(`attachments/${id}`)
		.then((r) => r.attachment)
		.catch((e) => {
			if (e instanceof ApiError && e.status === 404) return null;
			delete metas[id];
			throw e;
		});
	return metas[id];
}

/** After a removal: the cached row goes. */
export function forget(id: string) {
	metas[id] = Promise.resolve(null);
}

/** Photos the browser can redraw; a GIF would lose its frames. */
const SHRINKABLE = ['image/jpeg', 'image/png', 'image/webp', 'image/heic', 'image/heif'];
const JPEG_QUALITY = 0.86;
/** A photo already within the edge and under this size goes as it is. */
const SMALL_ENOUGH = 1_500_000;

/**
 * A photo redrawn to at most `edge` pixels on its long side, as a JPEG
 * with its orientation applied and its EXIF (place, device) left out.
 *
 * @param file - The picked file.
 * @param edge - The longest side, in pixels.
 * @returns The smaller photo, or `file` itself when it is not a photo,
 *   is small already, or the browser cannot decode it.
 */
export async function shrink(file: File, edge: number): Promise<File> {
	if (!SHRINKABLE.includes(file.type)) return file;
	let bitmap: ImageBitmap;
	try {
		bitmap = await createImageBitmap(file, { imageOrientation: 'from-image' });
	} catch {
		return file;
	}
	try {
		const long = Math.max(bitmap.width, bitmap.height);
		const png = file.type === 'image/png';
		if (long <= edge && (png || file.size <= SMALL_ENOUGH) && !file.type.startsWith('image/hei'))
			return file;
		const scale = Math.min(1, edge / long);
		const canvas = document.createElement('canvas');
		canvas.width = Math.max(1, Math.round(bitmap.width * scale));
		canvas.height = Math.max(1, Math.round(bitmap.height * scale));
		const ctx = canvas.getContext('2d');
		if (!ctx) return file;
		// NOTE: JPEG has no transparency: a see-through PNG gets white, not black
		ctx.fillStyle = '#fff';
		ctx.fillRect(0, 0, canvas.width, canvas.height);
		ctx.imageSmoothingQuality = 'high';
		ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
		const blob = await new Promise<Blob | null>((done) =>
			canvas.toBlob(done, 'image/jpeg', JPEG_QUALITY)
		);
		if (!blob || blob.size >= file.size) return file;
		const name = file.name.replace(/\.[^.]+$/, '') || 'photo';
		return new File([blob], `${name}.jpg`, { type: 'image/jpeg', lastModified: file.lastModified });
	} finally {
		bitmap.close();
	}
}

/**
 * Sends a file to the hub.
 *
 * @param file - The file.
 * @param options - The session it is attached in, a progress callback
 *   (0 to 1) and a signal that cancels the upload.
 * @returns The attachment's row.
 * @throws Unauthorized for a refused token, ApiError with the hub's
 *   reason (too large, the store is full), Error when the link broke.
 */
export function upload(
	file: File,
	options: { session?: string; onprogress?: (share: number) => void; signal?: AbortSignal } = {}
): Promise<Attachment> {
	return new Promise((resolve, reject) => {
		const xhr = new XMLHttpRequest();
		xhr.open('POST', `/api/v1/attachments${query({ name: file.name, session: options.session })}`);
		xhr.setRequestHeader('Authorization', `Bearer ${token()}`);
		xhr.setRequestHeader('Content-Type', 'application/octet-stream');
		xhr.upload.onprogress = (e) => {
			if (e.lengthComputable) options.onprogress?.(e.loaded / e.total);
		};
		xhr.onerror = () => reject(new Error('The upload did not reach the hub.'));
		xhr.onabort = () => reject(new DOMException('cancelled', 'AbortError'));
		xhr.onload = () => {
			let body: { attachment?: Attachment; error?: string } = {};
			try {
				body = JSON.parse(xhr.responseText);
			} catch {
				// NOTE: a proxy's own page (413 from nginx) is not JSON
			}
			if (xhr.status === 401) reject(new Unauthorized());
			else if (xhr.status >= 200 && xhr.status < 300 && body.attachment) resolve(body.attachment);
			else if (xhr.status === 413 && !body.error)
				reject(
					new ApiError(
						'The file is larger than the proxy in front of the hub lets through.',
						xhr.status
					)
				);
			else reject(new ApiError(body.error || `The hub answered ${xhr.status}.`, xhr.status));
		};
		options.signal?.addEventListener('abort', () => xhr.abort());
		xhr.send(file);
	});
}

/** A file in a prompt's tray: on its way to the hub, there, or refused. */
export type TrayItem = {
	key: string;
	name: string;
	bytes: number;
	/** What the browser says it is, for the icon until the hub decides. */
	type: string;
	state: 'uploading' | 'done' | 'failed';
	/** 0 to 1 while uploading. */
	progress: number;
	error?: string;
	attachment?: Attachment;
	/** A local picture of an image, while this page lives. */
	preview?: string;
};

const TRAY_KEY = 'ccdash.attach.';
/** Files one prompt may carry. */
export const TRAY_MAX = 10;

/** The files of the prompt being written in one field. */
export class Tray {
	items = $state<TrayItem[]>([]);
	/** Said under the field: too many files, a refused token. */
	notice = $state('');
	#aborts: Record<string, AbortController> = {};
	#n = 0;

	/**
	 * @param session - The session the field writes to, noted on the uploads.
	 * @param storeKey - The draft's key: uploaded files of an unsent prompt
	 *   survive a reload on this device under it.
	 */
	constructor(
		readonly session?: string,
		readonly storeKey?: string
	) {
		if (!browser || !storeKey) return;
		let ids: string[] = [];
		try {
			ids = JSON.parse(localStorage.getItem(TRAY_KEY + storeKey) ?? '[]');
		} catch {
			// a damaged entry is an empty tray
		}
		for (const id of ids) {
			attachmentOf(id)
				.then((a) => {
					if (!a || this.items.some((i) => i.attachment?.id === id)) return;
					this.items.push({
						key: `kept${this.#n++}`,
						name: a.name,
						bytes: a.bytes,
						type: a.mime,
						state: 'done',
						progress: 1,
						attachment: a
					});
				})
				.catch(() => {});
		}
	}

	/** Uploads are still running: the prompt waits for them. */
	get busy(): boolean {
		return this.items.some((i) => i.state === 'uploading');
	}

	/** The uploaded files' ids, in the tray's order. */
	get ids(): string[] {
		return this.items.flatMap((i) => (i.attachment ? [i.attachment.id] : []));
	}

	/** The prompt as sent: `text` and the uploaded files' markers. */
	compose(text: string): string {
		return compose(text, this.ids);
	}

	#keep() {
		if (!this.storeKey) return;
		const ids = this.ids;
		if (ids.length) localStorage.setItem(TRAY_KEY + this.storeKey, JSON.stringify(ids));
		else localStorage.removeItem(TRAY_KEY + this.storeKey);
	}

	/** Takes files in: photos are made smaller first unless Settings says not to. */
	add(files: Iterable<File>) {
		this.notice = '';
		for (const picked of files) {
			if (this.items.length >= TRAY_MAX) {
				this.notice = `A prompt carries at most ${TRAY_MAX} files.`;
				break;
			}
			const key = `up${this.#n++}`;
			const abort = new AbortController();
			this.#aborts[key] = abort;
			this.items.push({
				key,
				name: picked.name || 'pasted',
				bytes: picked.size,
				type: picked.type,
				state: 'uploading',
				progress: 0,
				preview: picked.type.startsWith('image/') ? URL.createObjectURL(picked) : undefined
			});
			const item = () => this.items.find((i) => i.key === key);
			(async () => {
				const file = prefs.attach.resize ? await shrink(picked, prefs.attach.edge) : picked;
				const it = item();
				if (!it) return;
				it.name = file.name || it.name;
				it.bytes = file.size;
				return upload(file, {
					session: this.session,
					signal: abort.signal,
					onprogress: (share) => {
						const it = item();
						if (it) it.progress = share;
					}
				});
			})()
				.then((attachment) => {
					const it = item();
					if (!it || !attachment) return;
					it.state = 'done';
					it.progress = 1;
					it.attachment = attachment;
					it.name = attachment.name;
					this.#keep();
				})
				.catch((e: Error) => {
					const it = item();
					if (!it || e.name === 'AbortError') return;
					it.state = 'failed';
					it.error =
						e instanceof Unauthorized ? 'This device is not signed in to the hub.' : e.message;
				})
				.finally(() => delete this.#aborts[key]);
		}
	}

	/** Takes a file out; one still uploading is cancelled, one on the hub
	 * leaves it (it was never sent). */
	remove(key: string) {
		this.#aborts[key]?.abort();
		const gone = this.items.find((i) => i.key === key);
		if (gone?.preview) URL.revokeObjectURL(gone.preview);
		if (gone?.attachment) {
			forget(gone.attachment.id);
			del(`attachments/${gone.attachment.id}`).catch(() => {});
		}
		this.items = this.items.filter((i) => i.key !== key);
		this.#keep();
	}

	/** After the prompt went out. */
	clear() {
		for (const key of Object.keys(this.#aborts)) this.#aborts[key].abort();
		for (const i of this.items) if (i.preview) URL.revokeObjectURL(i.preview);
		this.items = [];
		this.notice = '';
		this.#keep();
	}
}
