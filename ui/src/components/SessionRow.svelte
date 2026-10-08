<!-- Owns a session row's identity, status and accessible action trigger. -->
<script lang="ts">
  import type { Session } from '../gen/Session';
  import { isBusy, relativeTime } from '../lib/format';
  import { clockTime } from '../lib/account-format';
  import Icon from './Icon.svelte';
  let {
    session,
    current,
    onselect,
    onmenu,
  }: {
    session: Session;
    current: string | undefined;
    onselect: (id: string, right: boolean) => void;
    onmenu: (session: Session, event: MouseEvent) => void;
  } = $props();
  const goal = $derived(session.goal);
</script>

<div class="session-row">
  <button
    oncontextmenu={(e) => onmenu(session, e)}
    class="session"
    class:active={session.id === current}
    class:archived={session.archived}
    aria-current={session.id === current ? 'page' : undefined}
    title={`${session.title || 'New session'} · Alt-click: open on the right`}
    onclick={(e) => onselect(session.id, e.altKey)}
  >
    <span
      class="dot"
      title={session.status}
      class:running={isBusy(session.status)}
      class:attention={session.status === 'approval'}
    ></span>
    <span class="text"
      ><span class="title">{session.title || 'New session'}</span
      >{#if session.status === 'approval'}<small class="attention-label">approval</small
        >{:else if session.status === 'sleeping'}<small
          >sleeping until {session.status_until
            ? clockTime(session.status_until)
            : 'quota reset'}</small
        >{:else if goal}<small
          >{goal.state === 'active' ? `round ${goal.round}/${goal.max_rounds}` : goal.state}</small
        >{/if}</span
    >
    {#if session.pinned}<Icon name="pin" size={12} />{/if}{#if session.worktree}<span
        title="Own git worktree"><Icon name="branch" size={11} /></span
      >{/if}<time>{relativeTime(session.updated)}</time>
  </button><button
    class="more quiet"
    aria-label={`Session actions: ${session.title || 'New session'}`}
    title="More"
    onclick={(e) => onmenu(session, e)}>⋯</button
  >
</div>

<style>
  .session-row {
    display: flex;
    align-items: center;
    border-radius: var(--r-sm);
  }
  .session {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex: 1;
    min-width: 0;
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
  .archived {
    opacity: 0.65;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: var(--space-1);
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  small,
  time {
    color: var(--ink-4);
    font: 10px var(--mono);
  }
  small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    width: 5px;
    height: 5px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--ink-4);
  }
  .running {
    background: var(--baton);
  }
  .attention {
    box-shadow: 0 0 0 3px var(--baton-soft);
  }
  .attention-label {
    color: var(--baton);
  }
  .more {
    width: 24px;
    min-height: 30px;
    padding: 0;
    border: 0;
    font-size: 16px;
    color: var(--ink-4);
  }
</style>
