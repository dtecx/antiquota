<!-- Owns project groups, message search and session/project menu entry points. -->
<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import type { Workbench } from '../lib/workbench';
  import type { Accounts } from '../lib/accounts';
  import type { Login } from '../lib/login';
  import type { Api } from '../lib/api';
  import type { Panes } from '../lib/panes';
  import type { Connection } from '../lib/stream';
  import { minuteClock } from '../lib/workbench-lifecycle';
  import { isBusy, projectName, relativeTime } from '../lib/format';
  import { groupSessions, PINNED } from '../lib/navigation';
  import { preferences } from '../lib/preferences';
  import { createMessageSearch } from '../lib/message-search';
  import { createSidebarActions } from '../lib/sidebar-actions';
  import AccountPill from './AccountPill.svelte';
  import SessionRow from './SessionRow.svelte';
  import Icon from './Icon.svelte';
  import BrandMark from './BrandMark.svelte';
  let {
    workbench,
    connection,
    accounts,
    login,
    api,
    panes,
  }: {
    workbench: Workbench;
    connection: Connection;
    accounts: Accounts;
    login: Login;
    api: Api;
    panes: Panes;
  } = $props();
  const view = $derived(workbench.state);
  const prefs = preferences.state;
  const search = untrack(() => createMessageSearch(api)),
    found = search.state;
  const { sessionMenu, projectMenu, toggle } = untrack(() =>
    createSidebarActions(api, workbench, panes.select),
  );

  const sessions = $derived($view.snapshot?.sessions ?? []);
  const groups = $derived(
    groupSessions(sessions, $view.filter, {
      showArchived: $view.showArchived,
      current: $view.selected?.id ?? '',
    }),
  );
  const archived = $derived(sessions.filter((s) => s.archived).length);
  const filter = $derived($view.filter);
  $effect(() => search.query(filter));
  onDestroy(search.dispose);
  function select(id: string, right: boolean) {
    if (right) void panes.open(id);
    else {
      panes.focus('left');
      void panes.select(id);
    }
  }
</script>

<aside class:open={$view.sidebarOpen} aria-label="Sessions">
  <div class="brand-row" data-tauri-drag-region="deep">
    <div class="brand"><BrandMark />antiquota</div>
    <button
      class="icon quiet close-sidebar"
      aria-label="Close sessions"
      onclick={() => workbench.change({ sidebarOpen: false })}><Icon name="close" /></button
    >
  </div>
  <button class="new-session" onclick={() => workbench.openDialog('create')}
    ><Icon name="plus" /><span>New session</span><kbd>⌘ N</kbd></button
  >
  <div class="filter">
    <Icon name="search" /><input
      aria-label="Filter sessions"
      placeholder="Filter sessions…"
      value={$view.filter}
      oninput={(e) => workbench.change({ filter: e.currentTarget.value })}
    />
  </div>
  <nav aria-label="Projects and sessions">
    {#each groups as group (group.cwd)}
      {@const closed =
        !$view.filter.trim() &&
        $prefs.collapsed.includes(group.cwd) &&
        !group.sessions.some((s) => s.id === $view.selected?.id)}
      <section
        class="project"
        aria-label={group.cwd === PINNED ? 'Pinned' : projectName(group.cwd)}
      >
        <div class="project-head">
          <button
            class="project-name"
            oncontextmenu={(e) => {
              if (group.cwd !== PINNED) projectMenu(group.cwd, group.sessions, e);
            }}
            title={group.cwd === PINNED ? 'Pinned' : group.cwd}
            aria-expanded={!closed}
            onclick={() => toggle(group.cwd)}
            ><Icon name={group.cwd === PINNED ? 'pin' : 'folder'} /><span
              >{group.cwd === PINNED ? 'Pinned' : projectName(group.cwd)}</span
            ><span class="count">{group.sessions.length}</span
            >{#if closed && group.sessions.some((s) => isBusy(s.status))}<span
                class="working"
                title="A session here is working">●</span
              >{/if}<span class="chevron">{closed ? '›' : '⌄'}</span></button
          >
          {#if group.cwd !== PINNED}<button
              class="project-action"
              aria-label={`New session in ${projectName(group.cwd)}`}
              onclick={() => workbench.openDialog('create', { cwd: group.cwd, worktree: false })}
              >+</button
            ><button
              class="project-action"
              aria-label={`Project actions: ${projectName(group.cwd)}`}
              title="Project actions"
              onclick={(e) => projectMenu(group.cwd, group.sessions, e)}>⋯</button
            >{/if}
        </div>
        {#if !closed}{#each group.sessions as session (session.id)}<SessionRow
              {session}
              now={$minuteClock}
              current={$view.selected?.id}
              onselect={select}
              onmenu={sessionMenu}
            />{/each}{/if}
      </section>
    {/each}
    {#if !groups.length && $view.snapshot}<p class="empty muted">
        {$view.filter ? 'No matching sessions.' : 'Your sessions will appear here.'}
      </p>{/if}
    {#if $view.filter.trim().length >= 2}<section aria-label="In messages" class="message-results">
        <h3>In messages</h3>
        {#if $found.loading}<p class="empty" role="status">Searching…</p>{:else if $found.error}<p
            class="empty"
            role="alert"
          >
            {$found.error}
          </p>{:else}{#each $found.hits as hit (`${hit.id}:${hit.ts}`)}<button
              class="hit"
              title={hit.cwd}
              onclick={() => select(hit.id, false)}
              ><b>{hit.title || 'New session'}</b><time>{relativeTime(hit.ts, $minuteClock)}</time
              ><small>{hit.who === 'user' ? 'you' : 'agy'}: {hit.snippet}</small></button
            >{:else}<p class="empty">Nothing found.</p>{/each}{/if}
      </section>{/if}
    {#if archived}<button
        class="archived-toggle quiet"
        onclick={() => workbench.change({ showArchived: !$view.showArchived })}
        >{$view.showArchived ? 'Hide archived' : `Show archived (${archived})`}</button
      >{/if}
  </nav>
  <button class="quiet settings-link" onclick={() => accounts.open('general')}
    >Settings <kbd>⌘ ,</kbd></button
  >
  <AccountPill {accounts} {login} session={$view.selected} />
  <footer>
    <span class="connection-dot" class:live={connection === 'live'}></span><span
      >{connection === 'live'
        ? 'Engine connected'
        : connection === 'connecting'
          ? 'Connecting…'
          : 'Reconnecting…'}</span
    ><span class="version">{$view.snapshot?.engine.version ?? ''}</span>
  </footer>
</aside>

<style>
  .project-head {
    display: flex;
    align-items: center;
  }
  .project-name {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    text-align: left;
  }
  .project-name > span:first-of-type {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chevron {
    margin-left: auto;
  }
  .project-action {
    width: 24px;
    min-height: 30px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--ink-4);
  }
  .working {
    color: var(--baton);
  }
  .message-results h3 {
    font: 11px var(--mono);
    color: var(--ink-3);
    padding: var(--space-3) var(--space-2);
  }
  .hit {
    width: 100%;
    display: grid;
    grid-template-columns: 1fr auto;
    gap: var(--space-2);
    text-align: left;
    background: transparent;
    border: 0;
    font-size: 11px;
  }
  .hit b {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hit small {
    grid-column: 1 / -1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hit time,
  .hit small {
    font: 10px var(--mono);
    color: var(--ink-4);
  }
  .archived-toggle {
    font-size: 11px;
    width: 100%;
    text-align: left;
  }

  .settings-link {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--space-2) var(--space-4);
    font-size: 11px;
    color: var(--ink-3);
  }
  aside {
    width: var(--sidebar-width);
    flex-shrink: 0;
    background: var(--night-1);
    border-right: 1px solid var(--rule);
    display: flex;
    flex-direction: column;
  }
  .brand-row {
    height: 64px;
    padding: 0 var(--space-4);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-size: 19px;
    letter-spacing: -0.65px;
    font-weight: 600;
  }
  .new-session {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0 var(--space-3);
    text-align: left;
    background: var(--night-2);
  }
  .new-session kbd {
    margin-left: auto;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: var(--space-3) var(--space-4);
    color: var(--ink-3);
  }
  .filter input {
    padding: var(--space-2) 0;
    min-width: 0;
    border: none;
    background: transparent;
    font-size: 12px;
  }
  nav {
    flex: 1;
    overflow: auto;
    padding: 0 var(--space-2);
  }
  .project-name {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
    min-height: 36px;
    padding: 0 var(--space-2);
    color: var(--ink-2);
    font-size: 11px;
    font-weight: 500;
  }
  .count {
    color: var(--ink-4);
    font: 10px var(--mono);
  }
  .project {
    margin-bottom: var(--space-2);
  }
  .empty {
    padding: var(--space-4) var(--space-2);
    font-size: 12px;
  }
  footer {
    border-top: 1px solid var(--rule);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    color: var(--ink-3);
    font-size: 11px;
    min-height: 42px;
  }
  .connection-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--baton);
  }
  .connection-dot.live {
    background: var(--ok);
  }
  .version {
    margin-left: auto;
    color: var(--ink-4);
    font: 10px var(--mono);
  }
  .close-sidebar {
    display: none;
  }
  @media (max-width: 859px) {
    aside {
      position: absolute;
      inset: 0 auto 0 0;
      z-index: 10;
      display: none;
      box-shadow: 12px 0 32px rgb(0 0 0 / 30%);
    }
    aside.open {
      display: flex;
    }
    .close-sidebar {
      display: flex;
    }
  }
</style>
