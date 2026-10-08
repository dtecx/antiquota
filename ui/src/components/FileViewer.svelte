<!-- Owns the read-only file viewer: numbered lines, copy, and closing by button or backdrop. -->
<script lang="ts">
  import { tick } from 'svelte';
  import { viewer } from '../lib/git';
  import Icon from './Icon.svelte';
  let dialog: HTMLDialogElement;
  let copied = $state(false);
  const lines = $derived(($viewer?.text ?? '').replace(/\n$/, '').split('\n'));
  $effect(() => {
    if ($viewer) {
      copied = false;
      if (!dialog?.open) dialog?.showModal();
      const current = $viewer;
      if (current.line)
        void tick().then(() => {
          if ($viewer === current)
            dialog.querySelector('.line.highlight')?.scrollIntoView({ block: 'center' });
        });
    } else dialog?.close();
  });
  async function copy() {
    if (!$viewer) return;
    try {
      await navigator.clipboard.writeText($viewer.text);
      copied = true;
    } catch {
      copied = false;
    }
  }
</script>

<dialog
  bind:this={dialog}
  oncancel={() => viewer.set(null)}
  onclick={(event) => {
    if (event.target === dialog) viewer.set(null);
  }}
  aria-labelledby="viewer-title"
>
  {#if $viewer}
    <div class="heading">
      <h2 id="viewer-title" title={$viewer.path}>{$viewer.rel}</h2>
      <button class="small" onclick={copy}>{copied ? 'Copied' : 'Copy'}</button><button
        class="icon quiet"
        aria-label="Close file"
        onclick={() => viewer.set(null)}><Icon name="close" /></button
      >
    </div>
    <div class="body" aria-label="File contents">
      {#each lines as line, index (index)}<div
          class="line"
          class:highlight={$viewer.line === index + 1}
        >
          <span class="number">{index + 1}</span><span class="text">{line}</span>
        </div>{/each}
    </div>
  {/if}
</dialog>

<style>
  .line.highlight {
    background: var(--baton-soft);
  }
  dialog {
    width: min(960px, calc(100vw - 32px));
    padding: 0;
    border: 1px solid var(--rule-3);
    border-radius: var(--r-lg);
    background: var(--night);
    color: var(--ink);
    box-shadow: 0 24px 80px rgb(0 0 0 / 40%);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 45%);
  }
  .heading {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--rule);
  }
  h2 {
    flex: 1;
    min-width: 0;
    font: 500 12px var(--mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .small {
    font-size: 11px;
    min-height: 26px;
    padding: var(--space-1) var(--space-2);
  }
  .body {
    max-height: 70vh;
    overflow: auto;
    padding: var(--space-3) 0;
    background: var(--well);
    font: 11px/1.6 var(--mono);
  }
  .line {
    display: flex;
  }
  .number {
    flex-shrink: 0;
    width: 52px;
    padding-right: var(--space-3);
    text-align: right;
    color: var(--ink-4);
    user-select: none;
  }
  .text {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: var(--ink-2);
    padding-right: var(--space-4);
  }
</style>
