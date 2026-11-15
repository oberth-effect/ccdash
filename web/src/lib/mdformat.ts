// Markdown formatting for the composer's formatting bar: pure functions
// from the text and the selection to one replacement, so the editor applies
// each as a single undoable change.

/** One replacement: `from..to` becomes `insert`, then `select` (offsets from
 * `from`) is selected. */
export type Edit = { from: number; to: number; insert: string; select: [number, number] };

/**
 * Wraps the selection in a mark, or takes the mark away when it is there.
 *
 * @param text - The whole text.
 * @param from - Selection start.
 * @param to - Selection end; equal to `from` for a cursor.
 * @param open - The opening mark, e.g. `**`.
 * @param close - The closing mark; the opening one when omitted.
 * @returns The edit; the selection stays on the words, not the marks.
 * @example
 * wrap('a b', 2, 3, '**') // → { from: 2, to: 3, insert: '**b**', select: [2, 3] }
 */
export function wrap(text: string, from: number, to: number, open: string, close = open): Edit {
	const selected = text.slice(from, to);
	const outside =
		text.slice(from - open.length, from) === open && text.slice(to, to + close.length) === close;
	if (outside && from - open.length >= 0) {
		return {
			from: from - open.length,
			to: to + close.length,
			insert: selected,
			select: [0, selected.length]
		};
	}
	if (
		selected.length >= open.length + close.length &&
		selected.startsWith(open) &&
		selected.endsWith(close)
	) {
		const inner = selected.slice(open.length, selected.length - close.length);
		return { from, to, insert: inner, select: [0, inner.length] };
	}
	return {
		from,
		to,
		insert: open + selected + close,
		select: [open.length, open.length + selected.length]
	};
}

export type LineKind = 'bullet' | 'number' | 'quote' | 'heading';

const LINE_MARK: Record<LineKind, RegExp> = {
	bullet: /^(\s*)[-*+] /,
	number: /^(\s*)\d+[.)] /,
	quote: /^(\s*)> ?/,
	heading: /^(\s*)#{1,6} /
};

/**
 * Starts every selected line with a list, quote or heading mark, or takes
 * the marks away when every line has one. A bullet list becomes a numbered
 * one and back; blank lines are left alone.
 *
 * @param text - The whole text.
 * @param from - Selection start.
 * @param to - Selection end; a cursor takes its line.
 * @param kind - Which mark.
 * @returns The edit over the whole lines; a cursor ends up at the line's end.
 */
export function prefixLines(text: string, from: number, to: number, kind: LineKind): Edit {
	const start = text.lastIndexOf('\n', from - 1) + 1;
	const nl = text.indexOf('\n', to);
	const end = nl === -1 ? text.length : nl;
	const lines = text.slice(start, end).split('\n');
	const filled = lines.filter((l) => l.trim());
	const marked = filled.length > 0 && filled.every((l) => LINE_MARK[kind].test(l));
	let n = 0;
	const out = lines.map((line) => {
		if (!line.trim() && lines.length > 1) return line;
		if (marked) return line.replace(LINE_MARK[kind], '$1');
		const indent = /^\s*/.exec(line)![0];
		let rest = line.slice(indent.length);
		if (kind === 'bullet' || kind === 'number')
			rest = rest.replace(/^[-*+] /, '').replace(/^\d+[.)] /, '');
		if (kind === 'heading') rest = rest.replace(/^#{1,6} /, '');
		n += 1;
		const mark =
			kind === 'bullet' ? '- ' : kind === 'number' ? `${n}. ` : kind === 'quote' ? '> ' : '## ';
		return indent + mark + rest;
	});
	const insert = out.join('\n');
	return {
		from: start,
		to: end,
		insert,
		select: from === to ? [insert.length, insert.length] : [0, insert.length]
	};
}

/**
 * A markdown link from the selection: selected words become the label with
 * the address selected to type over; a selected address becomes the target
 * with the cursor in the label.
 *
 * @param text - The whole text.
 * @param from - Selection start.
 * @param to - Selection end.
 * @returns The edit.
 */
export function link(text: string, from: number, to: number): Edit {
	const selected = text.slice(from, to);
	if (/^(https?:\/\/|www\.)\S+$/.test(selected))
		return { from, to, insert: `[](${selected})`, select: [1, 1] };
	const insert = `[${selected}](url)`;
	return { from, to, insert, select: [selected.length + 3, selected.length + 6] };
}

/**
 * A fenced code block around the selection, on lines of its own.
 *
 * @param text - The whole text.
 * @param from - Selection start.
 * @param to - Selection end.
 * @returns The edit; the selection is the code, or the cursor sits inside.
 */
export function fence(text: string, from: number, to: number): Edit {
	const selected = text.slice(from, to);
	const before = from === 0 || text[from - 1] === '\n' ? '' : '\n';
	const after = to === text.length || text[to] === '\n' ? '' : '\n';
	const head = `${before}\`\`\`\n`;
	return {
		from,
		to,
		insert: `${head}${selected}\n\`\`\`${after}`,
		select: [head.length, head.length + selected.length]
	};
}
