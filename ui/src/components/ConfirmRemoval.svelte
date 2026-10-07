<!-- Owns explicit confirmation for removing only an antiquota account slot. -->
<script lang="ts">
  import type { Accounts } from '../lib/accounts';
  let { accounts }: { accounts: Accounts } = $props();
  const view = $derived(accounts.state);
  let dialog: HTMLDialogElement;
  $effect(() => {
    if ($view.remove) dialog?.showModal();
    else dialog?.close();
  });
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="remove-title"
  oncancel={() => accounts.requestRemove(null)}
>
  <h2 id="remove-title">Remove {$view.remove}?</h2>
  <p>Your agy CLI keeps its own sign-in. You can add the account again later.</p>
  {#if $view.error}<p class="error" role="alert">{$view.error}</p>{/if}
  <div class="actions">
    <button disabled={!!$view.busy} onclick={() => accounts.requestRemove(null)}>Cancel</button
    ><button class="danger" disabled={!!$view.busy} onclick={() => accounts.remove()}
      >{$view.busy === 'remove' ? 'Removing…' : 'Remove'}</button
    >
  </div>
</dialog>

<style>
  dialog {
    width: min(440px, calc(100vw - 32px));
    padding: var(--space-5);
    border: 1px solid var(--rule-3);
    border-radius: var(--r-md);
    color: var(--ink);
    background: var(--night-1);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 55%);
  }
  h2 {
    font-size: 15px;
    overflow-wrap: anywhere;
  }
  p {
    margin-top: var(--space-3);
    color: var(--ink-3);
    font-size: 12px;
  }
  p.error {
    color: var(--bad);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-5);
  }
  .danger {
    background: var(--bad-soft);
    color: var(--bad);
    border-color: var(--bad);
  }
</style>
