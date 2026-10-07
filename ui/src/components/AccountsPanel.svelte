<!-- Owns the Accounts settings surface; controls and data belong to focused stores. -->
<script lang="ts">
  import type { Accounts } from '../lib/accounts';
  import type { Login } from '../lib/login';
  import type { Session } from '../gen/Session';
  import Icon from './Icon.svelte';
  import AccountCard from './AccountCard.svelte';
  import RotationSettings from './RotationSettings.svelte';
  import RoutingLog from './RoutingLog.svelte';
  import PermissionsSettings from './PermissionsSettings.svelte';
  import type { PermissionsStore } from '../lib/permissions';
  let {
    accounts,
    login,
    sessions,
    permissions,
  }: { accounts: Accounts; login: Login; sessions: Session[]; permissions: PermissionsStore } =
    $props();
  const view = $derived(accounts.state);
  let dialog: HTMLDialogElement;
  $effect(() => {
    if ($view.open) {
      dialog?.showModal();
      void permissions.refresh();
    } else dialog?.close();
  });
</script>

<dialog
  bind:this={dialog}
  class="settings"
  aria-labelledby="accounts-title"
  oncancel={() => accounts.close()}
>
  {#if $view.open}
    <header>
      <div>
        <span class="eyebrow">SETTINGS</span>
        <h2 id="accounts-title">Accounts</h2>
      </div>
      <button class="icon quiet" aria-label="Close settings" onclick={() => accounts.close()}
        ><Icon name="close" /></button
      >
    </header>
    <div class="body">
      <div class="introduction">
        <p>
          Google accounts antiquota signs in separately from your agy CLI. Gemini and Claude/GPT
          quotas run out separately.
        </p>
        <div class="actions">
          <button disabled={!!$view.busy} onclick={() => accounts.probe()}>Probe now</button><button
            class="primary"
            onclick={() => login.open()}>Add account</button
          >
        </div>
      </div>
      {#if $view.error}<div class="error-message" role="alert">
          <p>{$view.error}</p>
          <button onclick={() => accounts.refresh()}>Retry</button>
        </div>{/if}
      {#if $view.feedback}<p class="feedback" role="status">{$view.feedback}</p>{/if}
      {#if $view.loading}<p class="loading" role="status">Refreshing accounts and settings…</p>{/if}
      <div class="cards" aria-busy={$view.loading}>
        {#each $view.accounts as account (account.slot)}<AccountCard
            {account}
            {accounts}
            signIn={() => login.open()}
          />{:else}{#if !$view.loading}<div class="empty">
              <h3>No accounts yet.</h3>
              <p>Add a Google account to begin. Your agy CLI keeps its own sign-in.</p>
            </div>{/if}{/each}
      </div>
      <RotationSettings {accounts} /><PermissionsSettings {permissions} /><RoutingLog
        entries={$view.routing}
        {sessions}
      />
    </div>
  {/if}
</dialog>

<style>
  dialog {
    width: min(1000px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    padding: 0;
    border: 1px solid var(--rule-3);
    border-radius: var(--r-lg);
    color: var(--ink);
    background: var(--night);
    box-shadow: 0 24px 80px rgb(0 0 0 / 40%);
    overflow: hidden;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 55%);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--space-4) var(--space-5);
    border-bottom: 1px solid var(--rule);
    background: var(--night-1);
  }
  .eyebrow {
    font: 9px var(--mono);
    letter-spacing: 0.08em;
    color: var(--ink-3);
  }
  h2 {
    font-size: 17px;
    font-weight: 500;
  }
  .body {
    max-height: calc(100vh - 140px);
    overflow-y: auto;
    display: grid;
    gap: var(--space-5);
    padding: var(--space-5);
  }
  .introduction {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--space-4);
  }
  .introduction p {
    max-width: 520px;
    font-size: 12px;
    color: var(--ink-3);
    line-height: 1.8;
  }
  .actions {
    display: flex;
    gap: var(--space-2);
    flex-shrink: 0;
  }
  button {
    font-size: 11px;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-3);
  }
  .error-message {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    color: var(--bad);
    background: var(--bad-soft);
    padding: var(--space-3);
    border-radius: var(--r-sm);
    font-size: 12px;
  }
  .error-message p {
    flex: 1;
    overflow-wrap: anywhere;
  }
  .feedback {
    font-size: 11px;
    color: var(--ok);
  }
  .loading,
  .empty p {
    font-size: 11px;
    color: var(--ink-3);
  }
  .empty {
    grid-column: 1 / -1;
    padding: var(--space-4);
    border: 1px solid var(--rule);
    border-radius: var(--r-md);
    display: grid;
    gap: var(--space-2);
  }
  .empty h3 {
    font-size: 13px;
  }
  @media (max-width: 760px) {
    dialog {
      width: calc(100vw - 16px);
      max-height: calc(100vh - 16px);
    }
    header,
    .body {
      padding: var(--space-4);
    }
    .body {
      max-height: calc(100vh - 100px);
    }
    .cards {
      grid-template-columns: 1fr;
    }
    .introduction {
      flex-direction: column;
    }
  }
</style>
