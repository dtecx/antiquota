<!-- Owns session identity, inline rename, status and basic metadata actions. -->
<script lang="ts">
  import { tick } from 'svelte';
  import type { Workbench } from '../lib/workbench';
  import Icon from './Icon.svelte';
  import { drawer } from '../lib/git';
  import { asWorktree } from '../lib/workbench';
  import { headerPath, headerStatus, headerUsage } from '../lib/header';
  let {
    workbench,
    onsplit,
    onclose,
    onswap,
  }: {
    workbench: Workbench;
    onsplit?: ((e: MouseEvent) => void) | undefined;
    onclose?: (() => void) | undefined;
    onswap?: (() => void) | undefined;
  } = $props();
  const view = $derived(workbench.state);
  const session = $derived($view.selected);
  const worktree = $derived(asWorktree($view.selected?.worktree));
  const path = $derived(headerPath(session?.cwd ?? '', worktree));
  const status = $derived(session ? headerStatus(session) : '');
  const usage = $derived(session ? headerUsage(session.usage) : '');
  let edit = $state<{ id: string; title: string; original: string } | null>(null);
  let titleInput = $state.raw<HTMLInputElement | null>(null);

  $effect(() => {
    if (edit && edit.id !== session?.id) edit = null;
  });

  async function beginRename() {
    if (!session || $view.mutating) return;
    const id = session.id;
    edit = { id, title: session.title, original: session.title };
    await tick();
    if (edit?.id !== id || session?.id !== id) return;
    titleInput?.focus();
    titleInput?.select();
  }

  function finishRename(save: boolean) {
    const pending = edit;
    if (save && pending && $view.mutating) {
      workbench.change({ feedback: 'Wait for the current action before saving the title.' });
      return;
    }
    edit = null;
    if (!save || !pending || pending.id !== session?.id) return;
    const title = pending.title.trim();
    if (title && title !== pending.original) void workbench.update({ title }, pending.id);
  }
</script>

<header data-tauri-drag-region="deep">
  {#if !onclose}<button
      class="icon quiet menu"
      aria-label="Open sessions"
      onclick={() => workbench.change({ sidebarOpen: true })}><Icon name="menu" /></button
    >{/if}
  <div class="identity">
    <h1>
      {#if edit && edit.id === session?.id}<input
          bind:this={titleInput}
          bind:value={edit.title}
          aria-label="Session title"
          onblur={() => finishRename(true)}
          onkeydown={(event) => {
            if (event.isComposing) return;
            if (event.key === 'Enter' || event.key === 'Escape') {
              event.preventDefault();
              event.stopPropagation();
              finishRename(event.key === 'Enter');
            }
          }}
        />{:else}<button
          class="title quiet"
          title={session ? 'Rename session' : undefined}
          disabled={!session || $view.mutating}
          onclick={beginRename}>{session ? session.title || 'Untitled' : 'New session'}</button
        >{/if}
    </h1>
    <p class="cwd" class:branch={!!worktree} title={path.tooltip}>
      {#if worktree}<Icon name="branch" size={11} />{/if}
      {path.label || 'Choose a project to start'}
    </p>
    {#if session && (status || usage || session.queue.length)}<div class="metadata">
        {#if status}<span class="status" class:attention={session.status === 'approval'}
            >{status}</span
          >{/if}
        {#if usage}<span class="tokens" title="Cumulative input and output tokens">{usage}</span
          >{/if}
        {#if session.queue.length}<span>{session.queue.length} queued</span>{/if}
      </div>{/if}
  </div>
  {#if $view.selected}
    <div class="controls">
      {#if onclose}<button class="quiet" aria-label="Swap sides" onclick={onswap}>⇄</button><button
          class="quiet"
          aria-label="Close side by side"
          onclick={onclose}>×</button
        >{:else if onsplit}<button
          class="quiet"
          aria-label="Split view"
          title="Split view (⌘\\)"
          onclick={onsplit}>◫</button
        >{/if}
      <button
        class="icon quiet"
        aria-label="Rename session"
        title="Rename session"
        disabled={$view.mutating}
        onclick={beginRename}><Icon name="edit" /></button
      >
      <button
        class="icon quiet"
        class:pinned={$view.selected.pinned}
        aria-label={$view.selected.pinned ? 'Unpin session' : 'Pin session'}
        title={$view.selected.pinned ? 'Unpin session' : 'Pin session'}
        onclick={() => workbench.update({ pinned: !$view.selected?.pinned })}
        disabled={$view.mutating}><Icon name="pin" /></button
      >{#if !onclose}<button
          class="icon quiet"
          class:pinned={$drawer.open}
          aria-label="Changes and files"
          title="Changes and files (⌘B)"
          aria-pressed={$drawer.open}
          onclick={() => drawer.update((d) => ({ ...d, open: !d.open }))}
          ><Icon name="panel" /></button
        >{/if}
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
  .title,
  h1 input {
    font: inherit;
    color: inherit;
    width: 100%;
    min-height: 0;
  }
  .title {
    display: block;
    text-align: left;
    border: 0;
    padding: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  h1 input {
    padding: 0 var(--space-1);
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
  .metadata {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    font: 10px/1.6 var(--mono);
    color: var(--ink-3);
  }
  .status {
    color: var(--baton);
  }
  .attention {
    color: var(--bad);
  }
  .pinned {
    color: var(--baton);
  }
  .menu {
    display: none;
  }
  @media (max-width: 859px) {
    .cwd,
    .tokens {
      display: none;
    }
    header {
      padding: var(--space-2);
    }
    .menu {
      display: flex;
    }
    .controls {
      gap: 0;
    }
  }
</style>
