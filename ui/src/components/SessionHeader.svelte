<!-- Owns current session identity, model selection, and basic metadata actions. -->
<script lang="ts">
  import type { Workbench } from '../lib/workbench';
  import Icon from './Icon.svelte';
  import { drawer } from '../lib/git';
  import { asWorktree } from '../lib/workbench';
  let { workbench }: { workbench: Workbench } = $props();
  const view = $derived(workbench.state);
  const models = $derived($view.snapshot?.models ?? []);
  const worktree = $derived(asWorktree($view.selected?.worktree));
</script>

<header>
  <button
    class="icon quiet menu"
    aria-label="Open sessions"
    onclick={() => workbench.change({ sidebarOpen: true })}><Icon name="menu" /></button
  >
  <div class="identity">
    <h1>{$view.selected?.title || 'New session'}</h1>
    {#if worktree}<p
        class="branch"
        title={`${worktree.path} · from ${worktree.base} in ${worktree.repo}`}
      >
        <Icon name="branch" size={11} />{worktree.branch}
      </p>{:else}<p title={$view.selected?.cwd}>
        {$view.selected?.cwd || 'Choose a project to start'}
      </p>{/if}
  </div>
  {#if $view.selected}
    <div class="controls">
      {#if models.length}<label class="model"
          ><span class="sr-only">Model</span><select
            aria-label="Model"
            disabled={$view.mutating || $view.selected.status === 'running'}
            value={$view.selected.model}
            onchange={(event) => workbench.update({ model: event.currentTarget.value })}
          >
            {#each models as model (model.id)}<option value={model.id}>{model.label}</option>{/each}
            {#if !models.some((model) => model.id === $view.selected?.model)}<option
                value={$view.selected.model}>{$view.selected.model}</option
              >{/if}
          </select></label
        >{:else}<span class="current-model" title={$view.selected.model}
          >{$view.selected.model}</span
        >{/if}
      <button
        class="icon quiet"
        aria-label="Rename session"
        title="Rename session"
        onclick={() => workbench.openDialog('rename')}><Icon name="edit" /></button
      >
      <button
        class="icon quiet"
        class:pinned={$view.selected.pinned}
        aria-label={$view.selected.pinned ? 'Unpin session' : 'Pin session'}
        title={$view.selected.pinned ? 'Unpin session' : 'Pin session'}
        onclick={() => workbench.update({ pinned: !$view.selected?.pinned })}
        disabled={$view.mutating}><Icon name="pin" /></button
      ><button
        class="icon quiet"
        class:pinned={$drawer.open}
        aria-label="Changes and files"
        title="Changes and files (⌘B)"
        aria-pressed={$drawer.open}
        onclick={() => drawer.update((d) => ({ ...d, open: !d.open }))}
        ><Icon name="panel" /></button
      >
    </div>
  {/if}
</header>

<style>
  header {
    min-height: 64px;
    padding: var(--space-3) var(--space-5);
    border-bottom: 1px solid var(--rule);
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .identity {
    flex: 1;
    min-width: 0;
  }
  h1 {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  p {
    font: 10px/1.8 var(--mono);
    color: var(--ink-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .branch {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--baton);
  }
  .controls {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  select {
    background: transparent;
    font-size: 11px;
    border-color: var(--rule);
    width: 180px;
  }
  .current-model {
    font: 10px var(--mono);
    color: var(--ink-3);
    padding: var(--space-2);
    max-width: 200px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pinned {
    color: var(--baton);
  }
  .menu {
    display: none;
  }
  @media (max-width: 760px) {
    header {
      padding: var(--space-2);
    }
    .menu {
      display: flex;
    }
    .current-model {
      display: none;
    }
    .model {
      display: none;
    }
    .controls {
      gap: 0;
    }
  }
</style>
