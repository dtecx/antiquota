<!-- Owns the side drawer beside the conversation: Changes and Files tabs. -->
<script lang="ts">
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import { drawer } from '../lib/git';
  import ChangesPanel from './ChangesPanel.svelte';
  import FilesPanel from './FilesPanel.svelte';
  import Icon from './Icon.svelte';
  let { workbench, api }: { workbench: Workbench; api: Api } = $props();
  const tabs = [
    { id: 'changes', label: 'Changes' },
    { id: 'files', label: 'Files' },
  ] as const;
</script>

{#if $drawer.open}
  <aside class="drawer" aria-label="Side panel">
    <div class="tabs" role="tablist">
      {#each tabs as tab (tab.id)}<button
          role="tab"
          aria-selected={$drawer.tab === tab.id}
          class:active={$drawer.tab === tab.id}
          onclick={() => drawer.update((d) => ({ ...d, tab: tab.id }))}>{tab.label}</button
        >{/each}
      <button
        class="icon quiet close"
        aria-label="Close side panel"
        onclick={() => drawer.update((d) => ({ ...d, open: false }))}><Icon name="close" /></button
      >
    </div>
    <div class="panel" role="tabpanel">
      {#if $drawer.tab === 'changes'}<ChangesPanel {workbench} {api} />{:else}<FilesPanel
          {workbench}
          {api}
        />{/if}
    </div>
  </aside>
{/if}

<style>
  .drawer {
    width: 380px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--rule);
    background: var(--night);
    min-height: 0;
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-height: 64px;
    padding: 0 var(--space-3);
    border-bottom: 1px solid var(--rule);
  }
  [role='tab'] {
    border: 0;
    background: none;
    font-size: 12px;
    color: var(--ink-3);
    min-height: 30px;
    padding: 0 var(--space-3);
    border-radius: var(--r-sm);
  }
  [role='tab'].active {
    color: var(--ink);
    background: var(--night-2);
  }
  .close {
    margin-left: auto;
  }
  .panel {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--space-4);
  }
  @media (max-width: 1100px) {
    .drawer {
      position: absolute;
      inset: 0 0 0 auto;
      width: min(380px, 100vw);
      z-index: 5;
      box-shadow: -16px 0 48px rgb(0 0 0 / 30%);
    }
  }
</style>
