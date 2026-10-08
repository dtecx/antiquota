<!-- Owns the shared browser/webview shell and connects stores to focused views. -->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { createApi } from './lib/api';
  import { createWorkbench } from './lib/workbench';
  import { createStream } from './lib/stream';
  import Sidebar from './components/Sidebar.svelte';
  import SessionDialog from './components/SessionDialog.svelte';
  import DropZone from './components/DropZone.svelte';
  import SettingsPanel from './components/SettingsPanel.svelte';
  import LoginDialog from './components/LoginDialog.svelte';
  import ConfirmRemoval from './components/ConfirmRemoval.svelte';
  import GoalDialog from './components/GoalDialog.svelte';
  import JobLog from './components/JobLog.svelte';
  import Drawer from './components/Drawer.svelte';
  import FileViewer from './components/FileViewer.svelte';
  import ShortcutDialog from './components/ShortcutDialog.svelte';
  import { shortcutsOpen } from './lib/commands';
  import { createComposerActions } from './lib/composer-actions';
  import ConfirmDialog from './components/ConfirmDialog.svelte';
  import { drawer } from './lib/git';
  import SessionPane from './components/SessionPane.svelte';
  import ContextMenu from './components/ContextMenu.svelte';
  import { contextMenu, openMenu } from './lib/context-menu';
  import { createPanes } from './lib/panes';
  import { groupSessions, splitChoices, stepSession } from './lib/navigation';
  import { clearTrays } from './lib/media';
  import { createAccounts } from './lib/accounts';
  import { createLogin } from './lib/login';
  import { createPermissions } from './lib/permissions';
  import { get } from 'svelte/store';
  import { notifications } from './lib/notifications';
  import { preferences } from './lib/preferences';
  import { createSettings } from './lib/settings';
  const token =
    document.querySelector<HTMLMetaElement>('meta[name="antiquota-token"]')?.content ?? '';
  const api = createApi(token);
  const workbench = createWorkbench(api, (id) =>
    history.replaceState(null, '', `#${encodeURIComponent(id)}`),
  );
  const view = workbench.state;
  const panes = createPanes(api, workbench),
    pane = panes.state,
    right = panes.right.state;
  const activeWorkbench = $derived($pane.active === 'right' && $pane.sid ? panes.right : workbench);
  $effect(() => clearTrays([$view.selected?.id ?? '', $right.selected?.id ?? '']));
  const settings = createSettings(api, () => {
    void workbench.refresh();
  });
  const accounts = createAccounts(api, settings);
  const login = createLogin(api, () => accounts.refresh());
  const permissions = createPermissions(api);
  const commandHost = {
    settings: accounts.open,
    login: () => login.open(),
    help: () => shortcutsOpen.set(true),
  };
  const actions = createComposerActions(api, workbench, commandHost);
  const rightActions = createComposerActions(api, panes.right, commandHost);
  $effect(() => {
    const session = $view.selected;
    document.title = `${session && session.status !== 'idle' ? '● ' : ''}${session?.title || 'antiquota'} · antiquota`;
  });
  const stream = createStream(
    api.streamUrl(),
    (event) => {
      workbench.onEvent(event);
      if (get(pane).sid) panes.right.onEvent(event);
      accounts.onEvent(event);
      const selected = get(view).selected;
      if (selected && event.sid === selected.id)
        notifications.event(event, selected.title || 'antiquota', get(preferences.state));
    },
    async () => {
      await workbench.refresh();
      if (get(pane).sid) await panes.right.refresh();
    },
  );
  const connection = stream.connection;
  onMount(() => {
    let previousAccounts: unknown;
    let previousDefaults: unknown;
    const unsubscribe = view.subscribe((value) => {
      panes.reconcile();
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
    void workbench.refresh(decodeURIComponent(location.hash.slice(1))).then(() => panes.restore());
    if (token && token !== '__ANTIQUOTA_TOKEN__') stream.open();
    return () => {
      unsubscribe();
      stream.close();
      login.dispose();
      settings.dispose();
      panes.dispose();
    };
  });
  function splitMenu(e: MouseEvent) {
    const choices = splitChoices($view.snapshot?.sessions ?? [], $view.selected?.id ?? '');
    openMenu(
      e,
      [
        ...choices.map((s) => ({
          label: `${s.status !== 'idle' ? '● ' : ''}${s.title || 'New session'}`,
          action: () => panes.open(s.id),
        })),
        ...($pane.sid ? [{ label: 'Close side by side', action: panes.close }] : []),
      ],
      'Show beside this one',
    );
  }
  function shortcut(event: KeyboardEvent) {
    if (event.defaultPrevented || document.querySelector('dialog[open]') || $contextMenu) return;
    const mod = event.metaKey || event.ctrlKey;
    if (mod && event.key === '/') {
      event.preventDefault();
      shortcutsOpen.set(true);
      return;
    }
    if (mod && event.key === '.') {
      event.preventDefault();
      void activeWorkbench.stop();
      return;
    }
    if (mod && event.key.toLowerCase() === 'j') {
      event.preventDefault();
      const id = get(activeWorkbench.state).selected?.id ?? '';
      document.getElementById(`prompt-${id}`)?.focus();
      return;
    }
    if (mod && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      workbench.change({ sidebarOpen: true });
      void tick().then(() => {
        const input = document.querySelector<HTMLInputElement>('[aria-label="Filter sessions"]');
        input?.focus();
        input?.select();
      });
      return;
    }
    if (mod && event.key === '\\') {
      event.preventDefault();
      if ($pane.sid) panes.close();
      else document.querySelector<HTMLButtonElement>('[aria-label="Split view"]')?.click();
      return;
    }
    if (mod && event.shiftKey && ['[', ']', '{', '}'].includes(event.key)) {
      event.preventDefault();
      const groups = groupSessions($view.snapshot?.sessions ?? [], $view.filter, {
        current: $view.selected?.id ?? '',
        showArchived: $view.showArchived,
      });
      const id = stepSession(
        groups,
        $view.selected?.id ?? '',
        [']', '}'].includes(event.key) ? 1 : -1,
      );
      if (id) {
        panes.focus('left');
        void panes.select(id);
      }
      return;
    }

    if ((event.metaKey || event.ctrlKey) && event.key === ',') {
      event.preventDefault();
      accounts.open('general');
      return;
    }
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'b' && !event.shiftKey) {
      event.preventDefault();
      drawer.update((d) => ({ ...d, open: !d.open }));
      return;
    }
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'n') {
      event.preventDefault();
      workbench.openDialog('create');
    }
  }
</script>

<svelte:window onkeydown={shortcut} />
<DropZone workbench={activeWorkbench} {api} />
<div class="workbench">
  <Sidebar {workbench} {accounts} {login} {api} {panes} connection={$connection} />
  <main>
    <SessionPane
      {workbench}
      {api}
      {login}
      {actions}
      connection={$connection}
      active={$pane.active === 'left'}
      onactivate={() => panes.focus('left')}
      onsplit={splitMenu}
    />
    {#if $pane.sid}<SessionPane
        workbench={panes.right}
        actions={rightActions}
        {api}
        {login}
        connection={$connection}
        active={$pane.active === 'right'}
        onactivate={() => panes.focus('right')}
        onclose={panes.close}
        onswap={() => void panes.swap()}
      />{/if}
  </main>
  <Drawer workbench={activeWorkbench} {api} />
  <SessionDialog {workbench} />
  <SessionDialog workbench={panes.right} />
  <GoalDialog workbench={activeWorkbench} />
  <JobLog />
  <FileViewer />
  <ConfirmDialog />
  <ShortcutDialog />
  {#if $contextMenu}{#key $contextMenu}<ContextMenu />{/key}{/if}
  <SettingsPanel
    {api}
    {workbench}
    {accounts}
    {login}
    {permissions}
    {settings}
    sessions={$view.snapshot?.sessions ?? []}
  />
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
    flex-direction: row;
  }
  main > :global(.session-pane) + :global(.session-pane) {
    border-left: 1px solid var(--rule);
  }
</style>
