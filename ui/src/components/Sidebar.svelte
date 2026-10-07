<!-- Owns project groups and session navigation in the compact warm workbench. -->
<script lang="ts">
  import type { Workbench } from '../lib/workbench';
  import type { Connection } from '../lib/stream';
  import { projectName, relativeTime } from '../lib/format';
  import Icon from './Icon.svelte';
  import { groupSessions } from '../lib/navigation';
  let { workbench, connection }: { workbench: Workbench; connection: Connection } = $props();
  const view = $derived(workbench.state);
  const groups = $derived(groupSessions($view.snapshot?.sessions ?? [], $view.filter));
</script>

<aside class:open={$view.sidebarOpen} aria-label="Sessions">
  <div class="brand-row">
    <div class="brand"><span class="baton" aria-hidden="true"></span>antiquota</div>
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
      oninput={(event) => workbench.change({ filter: event.currentTarget.value })}
    />
  </div>
  <nav aria-label="Projects and sessions">
    {#each groups as group (group.cwd)}
      <details class="project" open>
        <summary title={group.cwd}
          ><Icon name="folder" /><span>{projectName(group.cwd)}</span><span class="count"
            >{group.sessions.length}</span
          ></summary
        >
        <div class="sessions">
          {#each group.sessions as session (session.id)}
            <button
              class="session"
              class:active={session.id === $view.selected?.id}
              onclick={() => workbench.select(session.id)}
              aria-current={session.id === $view.selected?.id ? 'page' : undefined}
              title={session.title || 'New session'}
            >
              <span class="session-dot" class:running={session.status === 'running'}></span>
              <span class="session-title">{session.title || 'New session'}</span>
              {#if session.pinned}<Icon name="pin" size={12} />{:else}<time
                  >{relativeTime(session.updated)}</time
                >{/if}
            </button>
          {/each}
        </div>
      </details>
    {/each}
    {#if !groups.length && $view.snapshot}
      <p class="empty muted">
        {$view.filter ? 'No matching sessions.' : 'Your sessions will appear here.'}
      </p>
    {/if}
  </nav>
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
  .baton {
    width: 14px;
    height: 20px;
    border-left: 3px solid var(--baton);
    border-right: 3px solid var(--baton);
    transform: skew(-17deg);
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
  summary {
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
  summary::-webkit-details-marker {
    display: none;
  }
  summary::after {
    content: '›';
    color: var(--ink-4);
    margin-left: auto;
    transform: rotate(90deg);
  }
  details:not([open]) summary::after {
    transform: none;
  }
  summary > span:first-of-type {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .count {
    color: var(--ink-4);
    font: 10px var(--mono);
  }
  .project {
    margin-bottom: var(--space-2);
  }
  .session {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    min-height: 36px;
    padding: var(--space-2) var(--space-3);
    border-color: transparent;
    background: transparent;
    text-align: left;
    border-radius: var(--r-sm);
    color: var(--ink-2);
    font-size: 12px;
  }
  .session.active {
    background: var(--baton-soft);
    color: var(--ink);
    border-left-color: var(--baton);
  }
  .session-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .session-dot {
    width: 5px;
    height: 5px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--ink-4);
  }
  .session-dot.running {
    background: var(--baton);
  }
  time {
    color: var(--ink-4);
    font: 10px var(--mono);
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
  @media (max-width: 760px) {
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
