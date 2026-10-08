<!-- Owns settings navigation; focused stores retain drafts and surface mutations. -->
<script lang="ts">
  import { commands, shortcuts, shortcutLabel } from '../lib/commands';
  import type { Accounts } from '../lib/accounts';
  import type { Login } from '../lib/login';
  import type { Session } from '../gen/Session';
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import { type SettingsStore, settingsSections } from '../lib/settings';
  import SettingsDefaults from './SettingsDefaults.svelte';
  import SettingsAppearance from './SettingsAppearance.svelte';
  import UsagePanel from './UsagePanel.svelte';
  import StackSettings from './StackSettings.svelte';
  import EngineSettings from './EngineSettings.svelte';
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
    api,
    workbench,
    settings,
  }: {
    accounts: Accounts;
    login: Login;
    sessions: Session[];
    permissions: PermissionsStore;
    api: Api;
    workbench: Workbench;
    settings: SettingsStore;
  } = $props();
  const view = $derived(accounts.state);
  const setup = $derived(settings.state);
  const bench = $derived(workbench.state);
  const section = $derived($view.section);
  const open = $derived($view.open);
  const title = $derived(settingsSections.find(([id]) => id === section)?.[1] ?? 'General');
  $effect(() => {
    void section;
    settings.closeEditor();
  });
  let dialog: HTMLDialogElement;
  $effect(() => {
    if (open) {
      dialog?.showModal();
      void permissions.refresh();
    } else {
      dialog?.close();
      settings.closeEditor();
    }
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
        <h2 id="accounts-title">{title}</h2>
      </div>
      <button class="icon quiet" aria-label="Close settings" onclick={() => accounts.close()}
        ><Icon name="close" /></button
      >
    </header>
    <div class="layout">
      <nav aria-label="Settings sections">
        {#each settingsSections as [id, label] (id)}<button
            aria-label={label}
            class:active={section === id}
            aria-current={section === id ? 'page' : undefined}
            onclick={() => accounts.section(id)}
            >{label}{#if ['skills', 'plugins', 'rules', 'hooks', 'mcp'].includes(id)}<small
                >{$setup.data?.[
                  id === 'mcp' ? 'mcps' : (id as 'skills' | 'plugins' | 'rules' | 'hooks')
                ]?.filter((r) => r.here).length ?? 0}</small
              >{/if}{#if id === 'accounts' && $view.accounts.some((a) => a.verify)}<span
                class="attention">●</span
              >{/if}</button
          >{/each}
        <p title={$setup.data?.path}>stack: {$setup.data?.path ?? 'Reading…'}</p>
      </nav>
      <div class="body">
        {#if $setup.error}<p class="error-message" role="alert">{$setup.error}</p>{/if}
        {#if $setup.feedback}<p class="feedback" role="status">{$setup.feedback}</p>{/if}
        {#if $view.error}<div class="error-message" role="alert">
            <p>{$view.error}</p>
            <button onclick={() => accounts.refresh()}>Retry</button>
          </div>{/if}
        {#if section === 'accounts'}
          <div class="introduction">
            <p>
              Google accounts antiquota signs in separately from your agy CLI. Gemini and Claude/GPT
              quotas run out separately.
            </p>
            <div class="actions">
              <button disabled={!!$view.busy} onclick={() => accounts.probe()}>Probe now</button
              ><button class="primary" onclick={() => login.open()}>Add account</button>
            </div>
          </div>

          {#if $view.feedback}<p class="feedback" role="status">{$view.feedback}</p>{/if}
          {#if $view.loading}<p class="loading" role="status">
              Refreshing accounts and settings…
            </p>{/if}
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
          <RotationSettings {accounts} /><RoutingLog entries={$view.routing} {sessions} />
        {:else if section === 'general' || section === 'agents'}<SettingsDefaults
            {settings}
            {section}
            models={$bench.snapshot?.models ?? []}
          />
        {:else if section === 'appearance'}<SettingsAppearance />
        {:else if section === 'usage'}<UsagePanel {api} />
        {:else if section === 'permissions'}<button
            disabled={$setup.saving}
            onclick={async () => {
              if (await settings.copy('cli', 'permissions')) await permissions.refresh();
            }}>Copy from agy CLI</button
          ><PermissionsSettings {permissions} />
        {:else if section === 'engine'}<EngineSettings {api} {settings} />
        {:else if section === 'keyboard'}<h3>Keyboard</h3>
          <p>⌘ is Ctrl on Windows and Linux.</p>
          <h3>Keys</h3>
          <dl class="shortcuts">
            {#each shortcuts as [key, description] (key)}<dt>{shortcutLabel(key ?? '')}</dt>
              <dd>{description}</dd>{/each}
          </dl>
          <h3>Commands</h3>
          <dl class="shortcuts">
            {#each commands as [command, description] (command)}<dt>{command}</dt>
              <dd>{description}</dd>{/each}
          </dl>
        {:else}<StackSettings {settings} {section} />{/if}
        {#if $view.loading && section !== 'accounts'}<p class="loading" role="status">
            Reading antiquota's setup…
          </p>{/if}
      </div>
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
  .layout {
    display: grid;
    grid-template-columns: 180px minmax(0, 1fr);
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-4);
    border-right: 1px solid var(--rule);
    overflow: auto;
    max-height: calc(100vh - 140px);
  }
  nav button {
    text-align: left;
    font-size: 11px;
    border: 0;
    background: transparent;
    display: flex;
    justify-content: space-between;
    gap: var(--space-2);
  }
  nav button.active {
    color: var(--baton);
    background: var(--baton-soft);
  }
  nav p {
    font: 9px/1.6 var(--mono);
    color: var(--ink-4);
    overflow-wrap: anywhere;
    margin-top: var(--space-3);
  }
  .attention {
    color: var(--baton);
  }
  .shortcuts {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: var(--space-3);
    font-size: 12px;
  }
  .body > p {
    font-size: 12px;
    color: var(--ink-3);
    line-height: 1.7;
  }
  .body {
    max-height: calc(100vh - 140px);
    overflow-y: auto;
    display: grid;
    align-content: start;
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
    .layout {
      grid-template-columns: 1fr;
    }
    nav {
      flex-direction: row;
      max-height: 90px;
      border-right: 0;
      border-bottom: 1px solid var(--rule);
    }
    nav button {
      flex-shrink: 0;
    }
    nav p {
      display: none;
    }
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
