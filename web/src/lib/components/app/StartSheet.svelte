<script lang="ts">
	// Starts a background session (`claude --bg` on a connected host, at
	// the workspace's root, under a project with notes and a directory on
	// that host) or continues an ended one, on the host it ran on or
	// another that syncs it: host, workspace and project, the model, the
	// permission mode, the prompt. For an issue the prompt is written
	// already, to edit. Waits for the session to report and opens its page.
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import LoaderCircleIcon from '@lucide/svelte/icons/loader-circle';
	import { Unauthorized } from '#lib/api/client.js';
	import {
		fetchLaunch,
		fetchPrompts,
		fetchTargets,
		startLaunch,
		type Launch,
		type LaunchTarget
	} from '#lib/api/index.js';
	import type { StartPreset } from '#lib/starter.svelte.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Sheet from '#lib/components/ui/sheet/index.js';
	import PromptField from '#lib/components/session/PromptField.svelte';
	import { place } from '#lib/format.js';
	import { live } from '#lib/live/store.svelte.js';
	import { issuePrompt } from '#lib/notes.js';
	import { prefs } from '#lib/prefs.svelte.js';
	import { scope } from '#lib/scope.svelte.js';
	import { starter } from '#lib/starter.svelte.js';

	const MODELS: [string, string][] = [
		['', 'Default'],
		['fable', 'Fable'],
		['opus', 'Opus'],
		['sonnet', 'Sonnet'],
		['haiku', 'Haiku']
	];
	const MODES: [string, string][] = [
		['', 'Default'],
		['plan', 'Plan'],
		['acceptEdits', 'Accept edits']
	];
	/** The project select's value for no project (workspace-wide). */
	const ROOT = '/';
	const POLL_MS = 2000;

	let host = $state('');
	let ws = $state('');
	let project = $state('');
	let model = $state('');
	let mode = $state('');
	let text = $state('');
	let field = $state<PromptField | null>(null);
	/** Where sessions can start; null until loaded. */
	let targets = $state<LaunchTarget[] | null>(null);
	let pending = $state<string | null>(null);
	let busy = $state(false);
	let error = $state('');

	const resume = $derived(starter.preset.resume);
	const issue = $derived(starter.preset.issue);
	const draftKey = $derived(
		resume ? `resume:${resume.id}` : issue ? `issue:${issue.slug}` : 'start'
	);
	const connected = $derived((targets ?? []).map((t) => t.host));
	/** Where a session to resume can run: connected machines with its workspace. */
	const resumeHosts = $derived(
		resume
			? (targets ?? [])
					.filter((t) => t.workspaces.some((w) => w.name === resume.workspace))
					.map((t) => t.host)
			: []
	);
	const workspacesHere = $derived(targets?.find((t) => t.host === host)?.workspaces ?? []);
	const names = $derived(workspacesHere.find((w) => w.name === ws)?.projects);
	const launch = $derived<Launch | undefined>(pending ? live.launches.get(pending) : undefined);
	const ready = $derived(
		Boolean(host && ws && (resume || project) && text.trim() && !busy && !pending)
	);
	const touch = () => matchMedia('(pointer: coarse)').matches;

	// A fresh sheet: where sessions can start, then the preset, else the
	// last choice and the scope.
	$effect(() => {
		if (!starter.open) return;
		const preset = starter.preset;
		untrack(() => {
			model = prefs.start.model;
			mode = prefs.start.mode;
			pending = null;
			error = '';
			targets = null;
			fetchTargets()
				.then(({ hosts }) => {
					if (!starter.open) return;
					targets = hosts;
					pick(preset, hosts);
				})
				.catch((e) => {
					if (e instanceof Unauthorized) live.needToken = true;
					else error = (e as Error).message;
				});
		});
	});

	function pick(preset: StartPreset, hosts: LaunchTarget[]) {
		const r = preset.resume;
		const last = prefs.start.host;
		const has = (h: string | undefined) =>
			hosts.some((t) => t.host === h && (!r || t.workspaces.some((w) => w.name === r.workspace)));
		host =
			(r && has(r.host) ? r.host : has(last) ? last : hosts.find((t) => has(t.host))?.host) ?? '';
		const here = hosts.find((t) => t.host === host)?.workspaces ?? [];
		const want = r?.workspace ?? preset.ws ?? scope.current.ws[0] ?? '';
		ws = r?.workspace ?? (here.some((w) => w.name === want) ? want : (here[0]?.name ?? ''));
		const known = here.find((w) => w.name === ws)?.projects ?? [];
		project = r
			? (r.project ?? ROOT)
			: preset.project === null
				? ROOT
				: preset.project && known.includes(preset.project)
					? preset.project
					: '';
	}

	/** Another host: keep the workspace and project where that host has them. */
	function pickHost() {
		const here = workspacesHere;
		if (!here.some((w) => w.name === ws)) ws = here[0]?.name ?? '';
		const known = here.find((w) => w.name === ws)?.projects ?? [];
		if (project !== ROOT && !known.includes(project)) project = '';
	}

	$effect(() => {
		if (!starter.open) return;
		text = prefs.draft(draftKey) || (issue ? issuePrompt(issue.slug, issue.title) : '');
	});

	$effect(() => {
		if (starter.open) prefs.setDraft(draftKey, text);
	});

	// The launch moves on through the stream; polled too, for a missed update.
	$effect(() => {
		const id = pending;
		if (!id) return;
		const timer = setInterval(async () => {
			try {
				live.launches.set(id, (await fetchLaunch(id)).launch);
			} catch (e) {
				if (e instanceof Unauthorized) live.needToken = true;
			}
		}, POLL_MS);
		return () => clearInterval(timer);
	});

	$effect(() => {
		// NOTE: `launch` derives from `pending`: read it before clearing that
		const current = launch;
		if (!current) return;
		if (current.state === 'started') {
			pending = null;
			text = '';
			starter.open = false;
			goto(`/sessions/${current.session}`);
		} else if (current.state === 'failed') {
			pending = null;
			error = current.error;
		}
	});

	async function prompts() {
		return ws ? (await fetchPrompts({ workspace: ws, limit: 50 })).prompts : [];
	}

	async function start() {
		const body = text.trim();
		if (!ready) return;
		busy = true;
		error = '';
		try {
			const { launch } = await startLaunch({
				host,
				workspace: ws,
				project: project === ROOT ? undefined : project,
				prompt: body,
				model: model || undefined,
				permission_mode: mode || undefined,
				resume: resume?.id,
				issue: issue?.slug
			});
			prefs.setStart({ host, model, mode });
			field?.remember(body);
			// NOTE: the stream may have moved the launch on before this answer came
			if (!live.launches.has(launch.id)) live.launches.set(launch.id, launch);
			pending = launch.id;
		} catch (e) {
			if (e instanceof Unauthorized) live.needToken = true;
			else error = (e as Error).message;
		} finally {
			busy = false;
		}
	}

	const selectClass =
		'h-8 w-full min-w-0 rounded-lg border border-input bg-transparent px-2 text-sm text-foreground dark:bg-input/30';
</script>

<Sheet.Root bind:open={starter.open}>
	<Sheet.Content
		side={touch() ? 'bottom' : 'right'}
		class="flex max-h-[92vh] flex-col gap-3 overflow-y-auto p-4 sm:max-w-lg"
	>
		<Sheet.Header class="p-0 pr-8">
			<Sheet.Title
				>{resume
					? 'Resume the session'
					: issue
						? 'Work on this issue'
						: 'Start a session'}</Sheet.Title
			>
			<Sheet.Description>
				{#if issue}
					Starts a session on “{issue.title}”. It claims the issue when it reports, so the board
					shows it in progress.
				{:else if resume}
					Resumes “{resume.title}” in the background on {host || resume.host}, in
					{place(resume.cwd)}: its conversation goes on from where it stopped.
				{:else}
					Runs <code class="font-mono">claude --bg</code> on the host at the workspace's root (its harness
					and direnv), placed under the project: notes default to it. Its questions and approvals reach
					this device.
				{/if}
			</Sheet.Description>
		</Sheet.Header>

		{#if !targets}
			<p class="text-sm text-muted-foreground">Loading…</p>
		{:else if !connected.length}
			<p class="text-sm text-muted-foreground">
				No host is connected: sessions start on a workstation whose ccdash collector is up.
			</p>
		{:else}
			{#if !resume}
				<div class="grid grid-cols-1 gap-2 sm:grid-cols-3">
					<label class="flex min-w-0 flex-col gap-1 text-xs text-muted-foreground">
						Host
						<select class={selectClass} bind:value={host} onchange={pickHost}>
							{#each connected as h (h)}<option value={h}>{h}</option>{/each}
						</select>
					</label>
					<label class="flex min-w-0 flex-col gap-1 text-xs text-muted-foreground">
						Workspace
						<select class={selectClass} bind:value={ws} onchange={() => (project = '')}>
							{#each workspacesHere as w (w.name)}<option value={w.name}>{w.name}</option>{/each}
						</select>
					</label>
					<label class="flex min-w-0 flex-col gap-1 text-xs text-muted-foreground">
						Project
						<select class={selectClass} bind:value={project}>
							<option value="" disabled
								>{names?.length ? 'Pick a project' : 'No project here'}</option
							>
							{#each names ?? [] as p (p)}<option value={p}>{p}</option>{/each}
							<option value={ROOT}>(no project: the workspace itself)</option>
						</select>
					</label>
				</div>
			{:else}
				<div
					class="flex flex-col gap-1 text-xs text-muted-foreground"
					role="group"
					aria-label="Machine"
				>
					Machine
					<div class="flex flex-wrap gap-1.5">
						{#each resumeHosts as h (h)}
							<Button
								variant={host === h ? 'default' : 'outline'}
								size="sm"
								aria-pressed={host === h}
								onclick={() => (host = h)}
								>{h}{#if h === resume.host}<span class="opacity-70">· last ran here</span
									>{/if}</Button
							>
						{/each}
						{#if !connected.includes(resume.host)}
							<Button variant="outline" size="sm" disabled title="Its collector is not connected"
								>{resume.host} <span class="opacity-70">· offline</span></Button
							>
						{/if}
					</div>
				</div>
				{#if !resumeHosts.length}
					<p class="text-xs text-critical">
						No connected machine has the workspace {resume.workspace}.
					</p>
				{:else if host && host !== resume.host}
					<p class="text-xs text-muted-foreground">
						It last ran on {resume.host}; {host} fetches its transcript from the hub first, then resumes
						it as a copy under a new id (the old one stays as it was).
					</p>
				{/if}
			{/if}
			<div class="grid grid-cols-2 gap-2">
				<label class="flex min-w-0 flex-col gap-1 text-xs text-muted-foreground">
					Model
					<select class={selectClass} bind:value={model}>
						{#each MODELS as [value, name] (value)}<option {value}>{name}</option>{/each}
					</select>
				</label>
				<label class="flex min-w-0 flex-col gap-1 text-xs text-muted-foreground">
					Permission mode
					<select class={selectClass} bind:value={mode}>
						{#each MODES as [value, name] (value)}<option {value}>{name}</option>{/each}
					</select>
				</label>
			</div>
			<PromptField
				bind:this={field}
				bind:text
				{prompts}
				historyNote="The workspace's prompts."
				{host}
				workspace={ws || null}
				placeholder={resume ? 'What next?' : 'What should Claude do?'}
				label="Prompt"
				rows={3}
				disabled={busy || Boolean(pending)}
				onsubmit={start}
			>
				{#snippet actions()}
					<Button type="submit" size="sm" disabled={!ready}>
						{#if busy || pending}<LoaderCircleIcon class="animate-spin" />{/if}
						{resume ? 'Resume' : 'Start'}
					</Button>
				{/snippet}
			</PromptField>
			{#if pending}
				<p class="text-xs text-muted-foreground" role="status">
					Starting on {host}; the session's page opens when it reports.
				</p>
			{/if}
		{/if}
		{#if error}<p class="text-xs text-critical" role="alert">{error}</p>{/if}
	</Sheet.Content>
</Sheet.Root>
