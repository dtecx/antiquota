<!-- Owns native dialog presentation; editable fields and actions live in the store. -->
<script lang="ts">
  import type { Workbench } from '../lib/workbench';
  import type { Api } from '../lib/api';
  import { modes } from '../lib/composer-actions';
  import FolderBrowser from './FolderBrowser.svelte';
  import Icon from './Icon.svelte';
  let { workbench, api }: { workbench: Workbench; api: Api } = $props();
  const view = $derived(workbench.state);
  let dialog: HTMLDialogElement;
  $effect(() => {
    if ($view.dialog) dialog?.showModal();
    else dialog?.close();
  });
</script>

<dialog
  bind:this={dialog}
  oncancel={() => workbench.change({ dialog: null, error: '' })}
  aria-labelledby="session-dialog-title"
>
  {#if $view.dialog}
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void workbench.saveDialog();
      }}
    >
      <div class="heading">
        <h2 id="session-dialog-title">
          {$view.dialog.kind === 'create' ? 'New session' : 'Rename session'}
        </h2>
        <button
          type="button"
          class="icon quiet"
          aria-label="Close dialog"
          onclick={() => workbench.change({ dialog: null, error: '' })}
          ><Icon name="close" /></button
        >
      </div>
      <label
        >Session name<input
          aria-label="Session name"
          placeholder="What are you working on?"
          value={$view.dialog.title}
          oninput={(event) => workbench.dialogField('title', event.currentTarget.value)}
        /></label
      >
      {#if $view.dialog.kind === 'create'}<FolderBrowser
          {api}
          cwd={$view.dialog.cwd}
          sessions={$view.snapshot?.sessions ?? []}
          onchange={(path) => workbench.dialogField('cwd', path)}
        />
        <label
          >Permission mode<select
            aria-label="New session mode"
            value={$view.dialog.mode}
            onchange={(e) => workbench.dialogField('mode', e.currentTarget.value)}
          >
            {#each modes as mode (mode)}<option value={mode}>{mode}</option>{/each}
          </select></label
        >
        <label class="toggle"
          ><input
            type="checkbox"
            aria-label="Own git worktree"
            checked={$view.dialog.worktree}
            onchange={(event) => workbench.worktreeField(event.currentTarget.checked)}
          /><span
            >Own git worktree<small
              >A separate branch and folder, so parallel sessions never edit the same files.</small
            ></span
          ></label
        >{/if}
      {#if $view.error}<p class="error" role="alert">{$view.error}</p>{/if}
      <div class="actions">
        <button
          type="button"
          class="quiet"
          onclick={() => workbench.change({ dialog: null, error: '' })}>Cancel</button
        ><button class="primary" type="submit" disabled={$view.mutating}
          >{$view.mutating
            ? 'Saving…'
            : $view.dialog.kind === 'create'
              ? 'Create session'
              : 'Save name'}</button
        >
      </div>
    </form>
  {/if}
</dialog>

<style>
  dialog {
    width: min(440px, calc(100vw - 32px));
    padding: var(--space-5);
    border: 1px solid var(--rule-3);
    border-radius: var(--r-md);
    background: var(--night-1);
    color: var(--ink);
    box-shadow: 0 24px 80px rgb(0 0 0 / 40%);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 55%);
  }
  form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    font-size: 16px;
    font-weight: 600;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    font-size: 12px;
    color: var(--ink-2);
  }
  small {
    color: var(--ink-3);
    font-size: 11px;
  }
  .toggle {
    flex-direction: row;
    align-items: flex-start;
  }
  .toggle input {
    width: auto;
    margin-top: 2px;
  }
  .toggle span {
    display: grid;
    gap: var(--space-1);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }
</style>
