// The panel's help: one dialog with every key, opened with `?` anywhere
// outside a field, from the sidebar, or from the composer's menu.

/** A group of keys: where they work, then `[keys, what they do]` rows. */
export type KeyGroup = { title: string; note?: string; keys: [string, string][] };

export const KEY_GROUPS: KeyGroup[] = [
	{
		title: 'Everywhere',
		keys: [
			['Ctrl K', 'Search everything: sessions, notes, pages'],
			['?', 'This help'],
			['Esc', 'Close a dialog, a menu or a sheet']
		]
	},
	{
		title: 'A session',
		keys: [
			['i', 'Into the message field'],
			['Ctrl J', 'To the open question or permission prompt'],
			['Ctrl .', 'Stop the turn'],
			['Ctrl Shift 1–9', 'Send that quick reply']
		]
	},
	{
		title: 'Writing a prompt',
		note: 'Enter is always a new line, so a long prompt never goes out by accident.',
		keys: [
			['Ctrl Enter', 'Send'],
			['Ctrl Shift Enter', 'Next prompt: hand the text over without starting a turn'],
			['Ctrl ↑ ↓', 'Walk the prompt history'],
			['Ctrl R', 'Search the prompt history'],
			['Ctrl G', 'Edit in your editor on a machine'],
			['Ctrl Shift F', 'Expand the field to the screen, and back'],
			['/', 'Skills, at the start of the message'],
			['[[', 'Link a note'],
			['Ctrl V, or a drop', 'Attach a copied screenshot or file'],
			['Ctrl Shift D', 'Dictate: record, then Done; the words land at the cursor']
		]
	},
	{
		title: 'Formatting',
		note: 'The Aa button shows these as a bar above the field.',
		keys: [
			['Ctrl B', 'Bold'],
			['Ctrl I', 'Italic'],
			['Ctrl E', 'Code block around the selection'],
			['Ctrl P', 'Preview the markdown']
		]
	},
	{
		title: 'With vim keys',
		note: 'Off until Settings › Composer turns them on; the vimrc there adds maps.',
		keys: [
			['Esc', 'Normal mode'],
			[':w', 'Keep the draft now'],
			[':x  :q', 'Done: leave the field, or the expanded view'],
			[', then two characters', 'Hop: jump to a match in view']
		]
	},
	{
		title: 'Prompt history',
		keys: [
			['↑ ↓', 'Move through the prompts'],
			['Enter', 'Use the prompt'],
			['Shift Enter', 'Insert it at the cursor instead']
		]
	}
];

class Help {
	open = $state(false);

	show() {
		this.open = true;
	}
}

export const help = new Help();
