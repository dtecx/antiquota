<!-- Owns the Terminal tab: the selected session's shell with clear, restart and kill. -->
<script lang="ts">
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import { type Shell, fitShell, killShell, restartShell, shellFor } from '../lib/terminal';
  let { workbench, api }: { workbench: Workbench; api: Api } = $props();
  const view = $derived(workbench.state);
  const session = $derived($view.selected);
  let host: HTMLDivElement | undefined = $state();
  // Raw: the shell and its xterm instance change outside Svelte and must not be proxied.
  let shell = $state.raw<Shell | null>(null);
  let title = $state('Terminal');
  let alive = $state(false);
  let error = $state('');
  $effect(() => {
    const id = session?.id;
    const target = host;
    if (!id || !target) return;
    let cancelled = false;
    let observer: ResizeObserver | undefined;
    const refresh = () => {
      if (shell) {
        title = shell.title;
        alive = shell.alive;
      }
    };
    error = '';
    shellFor(id, api, api.termStreamUrl)
      .then((opened) => {
        if (cancelled) return;
        shell = opened;
        target.replaceChildren(opened.element);
        opened.listeners.add(refresh);
        refresh();
        observer = new ResizeObserver(() => fitShell(opened, api));
        observer.observe(target);
        fitShell(opened, api);
        opened.term.focus();
      })
      .catch((failure: unknown) => {
        if (!cancelled)
          error = `The terminal could not load: ${failure instanceof Error ? failure.message : String(failure)}`;
      });
    return () => {
      cancelled = true;
      observer?.disconnect();
      shell?.listeners.delete(refresh);
      target.replaceChildren();
    };
  });
</script>

<div class="terminal">
  {#if !session}
    <p class="empty">Open a session to use its terminal.</p>
  {:else}
    <div class="bar">
      <span class="title" class:exited={!alive} {title}>{title}</span>
      <button class="small" disabled={!shell} onclick={() => shell?.term.clear()}>Clear</button
      ><button
        class="small"
        disabled={!shell}
        onclick={() => shell && restartShell(shell, api, api.termStreamUrl)}>Restart</button
      ><button
        class="small"
        disabled={!shell || !alive}
        onclick={() => shell && killShell(shell, api)}>Kill</button
      >
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="host" bind:this={host} aria-label="Terminal output"></div>
  {/if}
</div>

<style>
  .terminal {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    height: 100%;
    min-height: 320px;
  }
  .empty {
    color: var(--ink-3);
    font-size: 12px;
  }
  .error {
    color: var(--bad);
    font-size: 12px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font: 10px var(--mono);
    color: var(--ink-3);
  }
  .title.exited {
    color: var(--ink-4);
  }
  .small {
    font-size: 11px;
    min-height: 26px;
    padding: var(--space-1) var(--space-2);
  }
  .host {
    flex: 1;
    min-height: 0;
    padding: var(--space-2);
    border-radius: var(--r-sm);
    background: var(--well);
    overflow: hidden;
  }
  .host :global(.xterm-host),
  .host :global(.xterm) {
    height: 100%;
  }
</style>
