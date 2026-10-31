// The notification kinds a device can mute (Settings › This device), with
// their labels; the names are the hub's `Category`.

import type { Category } from './api/types';

export const CATEGORIES: { id: Category; label: string; group: 'sessions' | 'workspace' }[] = [
	{ id: 'permission', label: 'Permission prompts', group: 'sessions' },
	{ id: 'question', label: 'Questions', group: 'sessions' },
	{ id: 'plan', label: 'Plans to approve', group: 'sessions' },
	{ id: 'turn', label: 'Finished turns', group: 'sessions' },
	{ id: 'ws_held', label: 'Held snapshots', group: 'workspace' },
	{ id: 'ws_conflict', label: 'Conflicts', group: 'workspace' },
	{ id: 'ws_new', label: 'New directories over the threshold', group: 'workspace' },
	{ id: 'ws_failing', label: 'Units failing for a while', group: 'workspace' }
];

/** Whether a subscription (null categories = all) wants a kind. */
export const wants = (categories: string[] | null | undefined, id: Category) =>
	categories == null || categories.includes(id);
