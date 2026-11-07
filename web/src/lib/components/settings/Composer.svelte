<script lang="ts">
	// Settings › Composer: how the field behaves on this device (vim keys,
	// line numbers, markdown, wrapping), the vimrc, which machine Edit in
	// nvim opens, the quick replies and the templates. Every choice here is
	// kept in this browser.
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { editHostFor } from '#lib/drafts.js';
	import { live } from '#lib/live/store.svelte.js';
	import { DEFAULT_QUICK, DEFAULT_VIMRC, prefs } from '#lib/prefs.svelte.js';

	let quickText = $state(prefs.quick.join('\n'));
	let templates = $state(prefs.templates.map((t) => ({ ...t })));
	let vimrcText = $state(prefs.vimrc);
	let vimrcErrors = $state<string[] | null>(null);
	/** Machines whose collector opens an editor from the panel. */
	const editors = $derived(live.hosts.filter((h) => h.edit));
	/** Where Ctrl G would open the editor from this device right now. */
	const opensOn = $derived(editHostFor(null));

	const selectClass =
		'h-8 rounded-lg border border-input bg-transparent px-2 text-sm dark:bg-input/30';

	function saveQuick() {
		prefs.setQuick(quickText.split('\n'));
		quickText = prefs.quick.join('\n');
	}

	async function saveVimrc() {
		const { checkVimrc } = await import('#lib/vim.js');
		vimrcErrors = checkVimrc(vimrcText);
		prefs.setVimrc(vimrcText);
	}

	function saveTemplates() {
		prefs.setTemplates(templates);
		templates = prefs.templates.map((t) => ({ ...t }));
	}
</script>

<section class="flex flex-col gap-2">
	<h2 class="text-base font-semibold">The field</h2>
	<p class="text-sm text-muted-foreground">
		Enter is a new line; Ctrl Enter sends. The field keeps its draft on the hub, so a prompt begun
		on the phone is on the desktop. These choices are this device's.
	</p>
	<div class="grid gap-3 sm:grid-cols-2">
		<label class="flex flex-col gap-1 text-sm">
			<span>Vim keys</span>
			<select
				class={selectClass}
				value={prefs.vim}
				onchange={(e) => {
					const v = e.currentTarget.value;
					if (v === 'auto' || v === 'on' || v === 'off') prefs.setVim(v);
				}}
			>
				<option value="auto">Auto: on with a mouse, off on touch</option>
				<option value="on">On</option>
				<option value="off">Off</option>
			</select>
		</label>
		<label class="flex flex-col gap-1 text-sm">
			<span>Line numbers</span>
			<select
				class={selectClass}
				value={prefs.editor.numbers}
				onchange={(e) => {
					const v = e.currentTarget.value;
					if (v === 'auto' || v === 'off' || v === 'absolute' || v === 'relative')
						prefs.setEditor({ numbers: v });
				}}
			>
				<option value="auto">Auto: relative with vim keys, else none</option>
				<option value="off">None</option>
				<option value="absolute">Absolute</option>
				<option value="relative">Relative, the current line absolute</option>
			</select>
		</label>
		<label class="flex items-center gap-2 text-sm">
			<input
				type="checkbox"
				class="size-4 accent-primary"
				checked={prefs.editor.markdown}
				onchange={(e) => prefs.setEditor({ markdown: e.currentTarget.checked })}
			/>
			Markdown set apart as you type (headings, emphasis, code, links)
		</label>
		<label class="flex items-center gap-2 text-sm">
			<input
				type="checkbox"
				class="size-4 accent-primary"
				checked={prefs.editor.wrap}
				onchange={(e) => prefs.setEditor({ wrap: e.currentTarget.checked })}
			/>
			Wrap long lines (off, they scroll sideways)
		</label>
	</div>
</section>

<section class="flex flex-col gap-2">
	<h2 class="text-base font-semibold">Vimrc</h2>
	<p class="text-sm text-muted-foreground">
		Ex commands the field's vim starts with, one per line: maps, as in Neovim, as far as the
		emulation follows. <code class="font-mono">:w</code> keeps the draft at once,
		<code class="font-mono">:x</code> is done editing.
	</p>
	<Textarea bind:value={vimrcText} rows={8} class="font-mono text-xs" spellcheck={false} />
	<div class="flex flex-wrap items-center gap-2">
		<Button size="sm" onclick={saveVimrc}>Save vimrc</Button>
		<Button
			variant="outline"
			size="sm"
			onclick={() => {
				vimrcText = DEFAULT_VIMRC;
				vimrcErrors = null;
			}}>Defaults</Button
		>
		{#if vimrcErrors !== null}
			<span class="text-xs {vimrcErrors.length ? 'text-warning' : 'text-muted-foreground'}">
				{vimrcErrors.length ? `Not taken: ${vimrcErrors.join('; ')}` : 'Saved; every line taken.'}
			</span>
		{/if}
	</div>
</section>

<section class="flex flex-col gap-2">
	<h2 class="text-base font-semibold">Edit in nvim</h2>
	<p class="text-sm text-muted-foreground">
		Ctrl G opens the draft in an editor on a machine, through its collector. Left on Auto, it is the
		machine this device sits on (the one whose collector connects from the same address), else the
		session's own, else the first that can.
	</p>
	<label class="flex max-w-md flex-col gap-1 text-sm">
		<span>Open the editor on</span>
		<select
			class={selectClass}
			value={prefs.editHost}
			onchange={(e) => prefs.setEditHost(e.currentTarget.value)}
		>
			<option value="">Auto</option>
			{#each editors as h (h.name)}
				<option value={h.name}>{h.name}{h.connected ? '' : ' (offline)'}</option>
			{/each}
		</select>
	</label>
	<p class="text-xs text-muted-foreground">
		{#if live.here}
			This device looks to be on <span class="font-medium text-foreground">{live.here}</span>.
		{:else}
			No machine's collector connects from this device's address: pick one above, or the editor
			opens on the session's machine.
		{/if}
		{#if opensOn}
			Right now Ctrl G would open on <span class="font-medium text-foreground">{opensOn}</span>.
		{/if}
	</p>
	{#if !editors.length}
		<p class="text-xs text-muted-foreground">
			No machine opens an editor yet: its collector needs
			<code class="font-mono">[edit] command</code> (the default runs
			<code class="font-mono">ghostty -e nvim</code>).
		</p>
	{/if}
</section>

<section class="flex flex-col gap-2">
	<h2 class="text-base font-semibold">Quick replies</h2>
	<label class="flex flex-col gap-1 text-sm">
		<span>One per line; Ctrl Shift 1–9 sends them</span>
		<Textarea bind:value={quickText} rows={5} class="font-mono text-xs" />
	</label>
	<div class="flex gap-2">
		<Button size="sm" onclick={saveQuick}>Save</Button>
		<Button variant="outline" size="sm" onclick={() => (quickText = DEFAULT_QUICK.join('\n'))}
			>Defaults</Button
		>
	</div>
</section>

<section class="flex flex-col gap-2">
	<h2 class="text-base font-semibold">Templates</h2>
	{#each templates as t, i (i)}
		<div class="flex flex-col gap-1 rounded-md border p-2">
			<Input bind:value={t.name} placeholder="Name" class="h-8" />
			<Textarea bind:value={t.text} rows={3} placeholder="Text" class="text-xs" />
			<div>
				<Button
					variant="ghost"
					size="xs"
					onclick={() => (templates = templates.filter((_, j) => j !== i))}>Remove</Button
				>
			</div>
		</div>
	{/each}
	<div class="flex gap-2">
		<Button
			variant="outline"
			size="sm"
			onclick={() => (templates = [...templates, { name: '', text: '' }])}>Add template</Button
		>
		<Button size="sm" onclick={saveTemplates}>Save templates</Button>
	</div>
</section>
