<!-- Owns the shared browser/webview shell and connects stores to focused views. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { createApi } from './lib/api';
  import { createWorkbench } from './lib/workbench';
  import { createStream } from './lib/stream';
  import Sidebar from './components/Sidebar.svelte';
  import SessionHeader from './components/SessionHeader.svelte';
  import SessionDialog from './components/SessionDialog.svelte';
  import Transcript from './components/Transcript.svelte';
  import Composer from './components/Composer.svelte';
  import Icon from './components/Icon.svelte';
  import AccountsPanel from './components/AccountsPanel.svelte';
  import LoginDialog from './components/LoginDialog.svelte';
  import ConfirmRemoval from './components/ConfirmRemoval.svelte';
  import { createAccounts } from './lib/accounts';
  import { createLogin } from './lib/login';
  import { createPermissions } from './lib/permissions';
  const token =
    document.querySelector<HTMLMetaElement>('meta[name="antiquota-token"]')?.content ?? '';
  const api = createApi(token);
  const workbench = createWorkbench(api, (id) =>
    history.replaceState(null, '', `#${encodeURIComponent(id)}`),
  );
  const view = workbench.state;
  const accounts = createAccounts(api);
  const login = createLogin(api, () => accounts.refresh());
  const permissions = createPermissions(api);
  const stream = createStream(
    api.streamUrl(),
    (event) => {
      workbench.onEvent(event);
      accounts.onEvent(event);
    },
    () => workbench.refresh(),
  );
  const connection = stream.connection;
  onMount(() => {
    let previousAccounts: unknown;
    let previousDefaults: unknown;
    const unsubscribe = view.subscribe((value) => {
      if (
        value.snapshot &&
        (previousAccounts !== value.snapshot.accounts ||
          previousDefaults !== value.snapshot.defaults)
      ) {
        previousAccounts = value.snapshot.accounts;
        previousDefaults = value.snapshot.defaults;
        accounts.acceptSnapshot(value.snapshot);
      }
    });
    void workbench.refresh(decodeURIComponent(location.hash.slice(1)));
    if (token && token !== '__ANTIQUOTA_TOKEN__') stream.open();
    return () => {
      unsubscribe();
      stream.close();
      login.dispose();
    };
  });
  function shortcut(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === ',') {
      event.preventDefault();
      accounts.open();
      return;
    }
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'n') {
      event.preventDefault();
      workbench.openDialog('create');
    }
  }
</script>

<svelte:window onkeydown={shortcut} />
<div class="workbench">
  <Sidebar {workbench} {accounts} {login} connection={$connection} />
  <main>
    <SessionHeader {workbench} />
    {#if $connection === 'reconnecting'}<div class="connection-banner" role="status">
        Engine disconnected. Reconnecting; your draft is kept.
      </div>{/if}
    {#if $view.error && !$view.dialog}<div class="error-banner" role="alert">
        <span>{$view.error}</span><button class="quiet" onclick={() => workbench.refresh()}
          >Retry</button
        ><button
          class="icon quiet"
          aria-label="Dismiss error"
          onclick={() => workbench.change({ error: '' })}><Icon name="close" /></button
        >
      </div>{/if}
    {#if $view.selected}<Transcript {workbench} {login} /><Composer
        {workbench}
        connected={$connection === 'live'}
      />
    {:else}<div class="welcome">
        <span class="eyebrow">YOUR WORKBENCH</span>
        <h1>A place for the next idea.</h1>
        <p>
          Create a session in a project folder.<br />The conversation and its tools stay together.
        </p>
        <button
          class="primary"
          onclick={() => workbench.openDialog('create')}
          disabled={$view.loading}><Icon name="plus" />New session</button
        >{#if !$view.snapshot?.accounts.length && !$view.loading}<button
            class="quiet"
            onclick={() => login.open()}>Add a Google account</button
          >{/if}
        {#if $view.loading}<small>Connecting to the engine…</small>{/if}
      </div>{/if}
  </main>
  <SessionDialog {workbench} />
  <AccountsPanel {accounts} {login} {permissions} sessions={$view.snapshot?.sessions ?? []} />
  <LoginDialog {login} {accounts} />
  <ConfirmRemoval {accounts} />
</div>

<style>
  .workbench {
    display: flex;
    height: 100%;
    position: relative;
  }
  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .connection-banner,
  .error-banner {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-5);
    font-size: 12px;
    flex-shrink: 0;
  }
  .connection-banner {
    background: var(--baton-soft);
    color: var(--baton);
  }
  .error-banner {
    background: var(--bad-soft);
    color: var(--bad);
  }
  .error-banner > span {
    flex: 1;
    overflow-wrap: anywhere;
  }
  .error-banner button {
    font-size: 11px;
    color: var(--bad);
  }
  .welcome {
    margin: auto;
    max-width: 560px;
    padding: var(--space-6);
    width: 100%;
  }
  .eyebrow {
    color: var(--baton);
    font: 10px var(--mono);
    letter-spacing: 0.08em;
  }
  h1 {
    font-size: 24px;
    font-weight: 500;
    letter-spacing: -0.5px;
    margin: var(--space-3) 0;
  }
  p {
    color: var(--ink-3);
    line-height: 1.8;
  }
  .welcome button {
    margin-top: var(--space-5);
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  small {
    display: block;
    color: var(--ink-3);
    margin-top: var(--space-3);
  }
  @media (max-width: 760px) {
    .connection-banner,
    .error-banner {
      padding: var(--space-2) var(--space-3);
    }
  }
</style>
