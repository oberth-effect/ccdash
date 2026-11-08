// The navigation registry: every area the sidebar shows. Adding an area
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
import HardDriveIcon from '@lucide/svelte/icons/hard-drive';
import HistoryIcon from '@lucide/svelte/icons/history';
import ListIcon from '@lucide/svelte/icons/list';
import SettingsIcon from '@lucide/svelte/icons/settings';
import type { Session } from './api/types';
import { live } from './live/store.svelte';
import { shown } from './session';

export type Area = {
	href: string;
	label: string;
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
		label: 'Now',
		icon: ActivityIcon,
		match: (path) => path === '/',
		badge: (sessions) =>
			sessions.filter((s) => s.activity === 'permission' || shown(s) === 'your_turn').length
	},
	{
		href: '/sessions',
		label: 'Sessions',
		icon: ListIcon,
		match: under('/sessions'),
		badge: (sessions) => sessions.filter((s) => s.activity !== 'gone').length
	},
	{
		href: '/projects',
		label: 'Projects',
		icon: FolderKanbanIcon,
		match: (path) => under('/projects')(path) || under('/notes')(path)
	},
	{ href: '/reports', label: 'Reports', icon: FileChartColumnIcon, match: under('/reports') },
	{ href: '/history', label: 'History', icon: HistoryIcon, match: under('/history') },
	{
		href: '/analytics/overview',
		label: 'Analytics',
		icon: BarChart3Icon,
		match: under('/analytics')
	},
	{ href: '/machines', label: 'Machines', icon: ComputerIcon, match: under('/machines') },
	{
		href: '/state',
		label: 'Claude state',
		icon: HardDriveIcon,
		match: under('/state'),
		on: () => live.anyHost('sync')
	},
	{
		href: '/workspace',
		label: 'Workspace',
		icon: FolderSyncIcon,
		match: under('/workspace'),
		on: () => Boolean(live.hub?.features.workspace)
	},
	{ href: '/settings', label: 'Settings', icon: SettingsIcon, match: under('/settings') }
];

/** The areas offered now. */
export const offered = (): Area[] => areas.filter((a) => a.on?.() ?? true);
