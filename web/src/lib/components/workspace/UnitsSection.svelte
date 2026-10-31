<script lang="ts">
	// Workspace › Units: the table with its filters, the new directories,
	// the held snapshots and the conflicts.
	import type { WorkspaceView } from '#lib/api/types.js';
	import HeldList from './HeldList.svelte';
	import NewDirs from './NewDirs.svelte';
	import UnitTable from './UnitTable.svelte';
	import WsConflicts from './WsConflicts.svelte';

	let { view, onchange }: { view: WorkspaceView; onchange: () => void } = $props();
</script>

<div class="flex flex-col gap-5">
	<UnitTable units={view.units} hosts={view.hosts.filter((h) => h.status)} />
	<NewDirs dirs={view.new_dirs} {onchange} />
	<HeldList held={view.held} {onchange} />
	<WsConflicts conflicts={view.conflicts} {onchange} />
</div>
