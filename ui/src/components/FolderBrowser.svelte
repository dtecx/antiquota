<!-- Owns the new-session folder input and its cancellable filesystem listing. -->
<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import type { Api } from '../lib/api';
  import type { Session } from '../gen/Session';
  import { createFolders, recentFolders } from '../lib/folders';
  let {
    api,
    cwd,
    sessions,
    onchange,
  }: {
    api: Api;
    cwd: string;
    sessions: Session[];
    onchange: (path: string) => void;
  } = $props();
  const folders = untrack(() => createFolders(api)),
    view = folders.state;
  const recent = $derived(recentFolders(sessions));
  onMount(() => folders.browse(''));
  onDestroy(folders.dispose);
  function go(path: string) {
    onchange(path);
    folders.browse(path);
  }
</script>

<label
  >Project folder<input
    aria-label="Project folder"
    placeholder="/path/to/project"
    value={cwd}
    required
    oninput={(event) => {
      onchange(event.currentTarget.value);
      folders.browse(event.currentTarget.value, true);
    }}
  /><small>Agent commands run inside this folder.</small></label
>
<div class="browser" aria-label="Folder browser">
  {#if $view.recent}{#each recent as path (path)}<button
        type="button"
        class="recent"
        title={path}
        onclick={() => go(path)}>★ {path}</button
      >{/each}{/if}
  {#if $view.listing}
    {@const listing = $view.listing}
    {#if listing.parent}<button
        type="button"
        aria-label="Parent folder"
        onclick={() => go(listing.parent!)}>↰ ..</button
      >{/if}
    {#each listing.dirs as name (name)}<button
        type="button"
        onclick={() => go(`${listing.path.replace(/[\\/]$/, '')}/${name}`)}>{name}/</button
      >{/each}
  {/if}
  {#if $view.error}<small role="status">{$view.error}</small>{/if}
</div>

<style>
  label {
    display: grid;
    gap: var(--space-2);
    font-size: 12px;
    color: var(--ink-2);
  }
  small {
    font-size: 11px;
    color: var(--ink-3);
  }
  .browser {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    max-height: 140px;
    overflow: auto;
  }
  button {
    min-height: 26px;
    padding: var(--space-1) var(--space-2);
    font: 10px var(--mono);
  }
  .recent {
    width: 100%;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
