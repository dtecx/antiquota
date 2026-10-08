<!-- Owns browsing the selected session's project folder: breadcrumbs, folders, file sizes,
     opening files in the viewer and mentioning them in the composer. -->
<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import { type Tree, breadcrumbs, humanSize, parentPath, viewFile } from '../lib/git';
  let { workbench, api }: { workbench: Workbench; api: Api } = $props();
  const view = $derived(workbench.state);
  const session = $derived($view.selected);
  let tree = $state<Tree | null>(null);
  let path = $state('');
  let error = $state('');
  let generation = 0,
    active = true;
  onDestroy(() => {
    active = false;
  });
  async function load(id: string, folder: string) {
    const own = ++generation;
    try {
      const result = await api.tree(id, folder);
      if (!active || own !== generation || session?.id !== id) return;
      tree = result;
      path = result.path;
      error = '';
    } catch (failure) {
      if (active && own === generation && session?.id === id)
        error = failure instanceof Error ? failure.message : 'The folder could not be read.';
    }
  }
  $effect(() => {
    const id = session?.id;
    tree = null;
    path = '';
    error = '';
    if (id) void load(id, '');
  });
  const join = (name: string) => (path ? `${path}/${name}` : name);
  async function open(name: string) {
    if (!session) return;
    const id = session.id,
      file = join(name);
    try {
      await viewFile(
        () => api.file(id, file),
        () => active && session?.id === id,
      );
    } catch (failure) {
      if (active && session?.id === id)
        error = failure instanceof Error ? failure.message : 'The file could not be opened.';
    }
  }
</script>

<div class="files">
  {#if !session}
    <p class="empty">Open a session to browse its folder.</p>
  {:else}
    <nav aria-label="Folder path">
      {#each breadcrumbs(path) as crumb, index (crumb.path)}{#if index}<span>/</span>{/if}<button
          class="crumb"
          onclick={() => load(session.id, crumb.path)}>{crumb.label}</button
        >{/each}
    </nav>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if tree}
      <ul aria-label="Folder contents">
        {#if path}<li>
            <button class="row" onclick={() => load(session.id, parentPath(path))}
              ><span class="icon">↰</span><span class="name">..</span></button
            >
          </li>{/if}
        {#each tree.entries as entry (entry.name)}
          <li class:dim={entry.dim}>
            {#if entry.dir}<button class="row" onclick={() => load(session.id, join(entry.name))}
                ><span class="icon">▸</span><span class="name">{entry.name}/</span></button
              >
            {:else}<button class="row" onclick={() => open(entry.name)}
                ><span class="icon">·</span><span class="name">{entry.name}</span><span class="size"
                  >{humanSize(entry.size)}</span
                ></button
              ><button
                class="mention"
                title="Mention in the message"
                aria-label={`Mention ${entry.name}`}
                onclick={() => workbench.mention(join(entry.name))}>@</button
              >{/if}
          </li>
        {:else}
          <li class="empty-folder">Empty folder</li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .files {
    display: grid;
    gap: var(--space-3);
    align-content: start;
  }
  .empty,
  .empty-folder {
    color: var(--ink-3);
    font-size: 12px;
  }
  .empty-folder {
    padding: var(--space-3);
  }
  .error {
    color: var(--bad);
    font-size: 12px;
  }
  nav {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1);
    font: 10px var(--mono);
    color: var(--ink-4);
  }
  .crumb {
    border: 0;
    background: none;
    padding: 0;
    min-height: 0;
    font: inherit;
    color: var(--ink-2);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
  }
  li {
    display: flex;
    align-items: center;
  }
  li + li {
    border-top: 1px solid var(--rule);
  }
  li.dim {
    opacity: 0.55;
  }
  .row {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    border: 0;
    border-radius: 0;
    background: none;
    padding: var(--space-2);
    min-height: 30px;
    text-align: left;
    font-size: 12px;
  }
  .row:hover {
    background: var(--night-2);
  }
  .icon {
    width: 12px;
    color: var(--ink-4);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
  }
  .size {
    font: 10px var(--mono);
    color: var(--ink-4);
  }
  .mention {
    border: 0;
    background: none;
    min-height: 0;
    padding: 0 var(--space-2);
    font: 11px var(--mono);
    color: var(--ink-3);
  }
</style>
