// The navigation registry: every area the sidebar shows, in three groups
// set apart by a line: the work (what runs, what is known), what is kept
// and measured around it, and the panel's own settings. Adding an area
// is one entry here plus its route folder. An area whose feature is off
// everywhere stays out of the sidebar and the palette (its route still
// answers).

import type { Component } from 'svelte';
import ActivityIcon from '@lucide/svelte/icons/activity';
import BarChart3Icon from '@lucide/svelte/icons/bar-chart-3';
import ComputerIcon from '@lucide/svelte/icons/computer';
import FileChartColumnIcon from '@lucide/svelte/icons/file-chart-column';
import FolderKanbanIcon from '@lucide/svelte/icons/folder-kanban';
import FolderSyncIcon from '@lucide/svelte/icons/folder-sync';
import FilesIcon from '@lucide/svelte/icons/files';
import HardDriveIcon from '@lucide/svelte/icons/hard-drive';
import HistoryIcon from '@lucide/svelte/icons/history';
import ListIcon from '@lucide/svelte/icons/list';
import SettingsIcon from '@lucide/svelte/icons/settings';
import type { Session } from './api/types';
import { live } from './live/store.svelte';
import { shown } from './session';

/** `work`: sessions and knowledge; `around`: files, figures, machines;
 * `panel`: its settings. */
export type Group = 'work' | 'around' | 'panel';

export type Area = {
	href: string;
	label: string;
	group: Group;
	icon: Component;
	/** Whether a path belongs to the area. */
	match: (path: string) => boolean;
	/** A count shown next to the entry. */
	badge?: (sessions: Session[]) => number;
	/** Whether the area is offered at all; always without. */
	on?: () => boolean;
};

const under = (prefix: string) => (path: string) =>
	path === prefix || path.startsWith(`${prefix}/`);

export const areas: Area[] = [
	{
		href: '/',
		group: 'work',
		label: 'Now',
		icon: ActivityIcon,
		match: (path) => path === '/',
		badge: (sessions) =>
			sessions.filter((s) => s.activity === 'permission' || shown(s) === 'your_turn').length
	},
	{
		href: '/sessions',
		group: 'work',
		label: 'Sessions',
		icon: ListIcon,
		match: under('/sessions'),
		badge: (sessions) => sessions.filter((s) => s.activity !== 'gone').length
	},
	{
		href: '/projects',
		group: 'work',
		label: 'Projects',
		icon: FolderKanbanIcon,
		match: (path) => under('/projects')(path) || under('/notes')(path)
	},
	{
		href: '/reports',
		group: 'work',
		label: 'Reports',
		icon: FileChartColumnIcon,
		match: under('/reports')
	},
	{
		href: '/files',
		group: 'around',
		label: 'Files',
		icon: FilesIcon,
		match: under('/files'),
		on: () => Boolean(live.hub?.features.attachments)
	},
	{
		href: '/history',
		group: 'around',
		label: 'History',
		icon: HistoryIcon,
		match: under('/history')
	},
	{
		href: '/analytics/overview',
		group: 'around',
		label: 'Analytics',
		icon: BarChart3Icon,
		match: under('/analytics')
	},
	{
		href: '/machines',
		group: 'around',
		label: 'Machines',
		icon: ComputerIcon,
		match: under('/machines')
	},
	{
		href: '/state',
		group: 'around',
		label: 'Claude state',
		icon: HardDriveIcon,
		match: under('/state'),
		on: () => live.anyHost('sync')
	},
	{
		href: '/workspace',
		group: 'around',
		label: 'Workspace',
		icon: FolderSyncIcon,
		match: under('/workspace'),
		on: () => Boolean(live.hub?.features.workspace)
	},
	{
		href: '/settings',
		group: 'panel',
		label: 'Settings',
		icon: SettingsIcon,
		match: under('/settings')
	}
];

/** The areas offered now. */
export const offered = (): Area[] => areas.filter((a) => a.on?.() ?? true);
