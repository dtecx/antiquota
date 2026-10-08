<!-- Renders one session pane; selection, draft and transcript state belong to its workbench. -->
<script lang="ts">
  import type { Workbench } from '../lib/workbench';
  import type { Api } from '../lib/api';
  import type { Login } from '../lib/login';
  import type { ComposerActions } from '../lib/composer-actions';
  import type { Connection } from '../lib/stream';
  import SessionHeader from './SessionHeader.svelte';
  import Transcript from './Transcript.svelte';
  import GoalBar from './GoalBar.svelte';
  import JobBar from './JobBar.svelte';
  import Composer from './Composer.svelte';
  import Icon from './Icon.svelte';
  import { revising } from '../lib/goals';
  let {
    workbench,
    api,
    login,
    connection,
    active = true,
    onactivate,
    actions,
    onsplit,
    onclose,
    onswap,
  }: {
    workbench: Workbench;
    api: Api;
    login: Login;
    connection: Connection;
    active?: boolean;
    onactivate: () => void;
    actions: ComposerActions;
    onsplit?: (e: MouseEvent) => void;
    onclose?: () => void;
    onswap?: () => void;
  } = $props();
  const view = $derived(workbench.state);
</script>

<section
  class="session-pane"
  aria-label={onclose ? 'Right session' : 'Primary session'}
  onfocusin={onactivate}
>
  <SessionHeader {workbench} {onsplit} {onclose} {onswap} />
  {#if connection === 'reconnecting'}<div class="connection-banner" role="status">
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
  {#if $view.selected}<Transcript {workbench} {login} {api} /><GoalBar
      {workbench}
      onrevise={() => {
        onactivate();
        revising.set(true);
      }}
    /><JobBar {workbench} />
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
  <Composer {workbench} {api} {active} {actions} connected={connection === 'live'} />
</section>

<style>
  .session-pane {
    flex: 1;
    min-width: 0;
    min-height: 0;
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
