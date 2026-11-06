// The start sheet's state: opened from Now (the scope's workspace), a
// project page (its project), an ended session's page (continuing it) or
// an open issue (a session to work on it).

import type { PastSession, Session } from './api/types';
import { label } from './session';

/** A session to continue: where it last ran and under what. */
export type Resumable = {
	id: string;
	host: string;
	cwd: string;
	workspace: string | null;
	project: string | null;
	title: string;
};

export const resumableSession = (s: Session): Resumable => ({
	id: s.id,
	host: s.host,
	cwd: s.cwd,
	workspace: s.workspace ?? null,
	project: s.project ?? null,
	title: label(s)
});

export const resumablePast = (s: PastSession): Resumable => ({
	id: s.id,
	host: s.host,
	cwd: s.cwd,
	workspace: s.workspace,
	project: s.project,
	title: s.title || s.first_prompt || s.id.slice(0, 8)
});

export type StartPreset = {
	ws?: string | null;
	/** Null is the workspace's own directory. */
	project?: string | null;
	/** The ended session to continue: its host, workspace and project. */
	resume?: Resumable;
	/** The open issue the session is for: it claims it when it reports. */
	issue?: { slug: string; title: string };
};

class Starter {
	open = $state(false);
	preset = $state<StartPreset>({});

	show(preset: StartPreset = {}) {
		this.preset = preset;
		this.open = true;
	}
}

export const starter = new Starter();
