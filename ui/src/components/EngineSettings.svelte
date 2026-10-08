<!-- Presents engine identity and deliberate lifecycle actions. -->
<script lang="ts">
  import { native, restartEngine, openNativePath, appInformation } from '../lib/native';
  import { onMount } from 'svelte';
  import type { Api } from '../lib/api';
  import type { SettingsStore } from '../lib/settings';
  import type { EngineResponse } from '../gen/EngineResponse';
  import { ask } from '../lib/confirm';
  let { api, settings }: { api: Api; settings: SettingsStore } = $props();
  let engine = $state<EngineResponse | null>(null),
    log = $state<string | null>(null),
    feedback = $state(''),
    desktopVersion = $state(''),
    restarting = $state(false);
  let active = true;
  const fail = (error: unknown) => {
    if (active) settings.fail(error);
  };
  onMount(() => {
    if (native)
      void appInformation()
        .then((info) => {
          if (active) desktopVersion = `${info.version} · ${info.os}`;
        })
        .catch(fail);
    void api
      .engine()
      .then((value) => {
        if (active) engine = value;
      })
      .catch(fail);
    return () => {
      active = false;
    };
  });
  async function showLog() {
    try {
      const result = await api.engineLog();
      if (active) log = result.text;
    } catch (error) {
      fail(error);
    }
  }
  async function restart() {
    if (restarting) return;
    restarting = true;
    try {
      if (
        !(await ask(
          'Restart the engine?',
          'Running turns stop and goals pause; background jobs keep running. The desktop app starts the engine again. In a browser, restart antiquota serve.',
          { ok: 'Restart' },
        )) ||
        !active
      )
        return;
      if (native) await restartEngine();
      else await api.shutdown();
      if (active) feedback = native ? 'Engine restarted.' : 'Engine restarting…';
    } catch (error) {
      fail(error);
    } finally {
      restarting = false;
    }
  }
  async function replace() {
    if (
      (await ask(
        'Replace antiquota’s private configuration?',
        'Edits made here are lost; conversations stay. Your agy CLI and IDE remain separate.',
        { ok: 'Replace', danger: true },
      )) &&
      active
    )
      await settings.copy('cli', 'everything');
  }
</script>

<section>
  {#if engine}<dl>
      {#each [['Version', engine.version], ['Engine', `${engine.url} · pid ${engine.pid} · ${engine.by}`], ['Data', engine.base], ['Stack', engine.stack], ['agy', engine.agy || 'not found'], ['Permissions', engine.hook ? 'antiquota decides each tool call (hook on)' : 'agy’s own headless rules (hook off)'], ['Sessions', `${engine.sessions} · ${engine.accounts} accounts`]] as [label, value] (label)}<dt
        >
          {label}
        </dt>
        <dd>{value}</dd>{/each}
    </dl>
    {#if desktopVersion}<p>Desktop {desktopVersion}</p>{/if}
    <div class="actions">
      {#if native}<button onclick={() => openNativePath(engine!.base).catch(fail)}
          >Data directory</button
        >{/if}
      <button onclick={showLog}>Engine log</button><button onclick={restart} disabled={restarting}
        >Restart engine</button
      >
    </div>
    <h3>antiquota stack</h3>
    <p>
      Every account uses antiquota’s private rules, skills, hooks, servers, plugins and permissions.
      These edits never change your agy CLI or IDE setup.
    </p>
    <button onclick={replace}>Copy everything again from agy CLI</button>{:else}<p>
      Reading engine details…
    </p>{/if}{#if log !== null}<div class="log">
      <header>
        <h3>Engine log</h3>
        <button onclick={() => (log = null)}>Close log</button>
      </header>
      <pre>{log}</pre>
    </div>{/if}{#if feedback}<p role="status">{feedback}</p>{/if}
</section>

<style>
  section {
    display: grid;
    gap: var(--space-4);
  }
  dl {
    display: grid;
    grid-template-columns: 90px 1fr;
    gap: var(--space-3);
    font-size: 12px;
  }
  dt {
    color: var(--ink-3);
  }
  dd {
    overflow-wrap: anywhere;
    font-family: var(--mono);
    font-size: 11px;
  }
  .actions,
  header {
    display: flex;
    gap: var(--space-2);
    justify-content: space-between;
  }
  button {
    font-size: 11px;
  }
  h3 {
    font-size: 13px;
    font-weight: 500;
  }
  p {
    font-size: 11px;
    color: var(--ink-3);
    line-height: 1.7;
  }
  .log {
    padding: var(--space-4);
    background: var(--well);
    border-radius: var(--r-sm);
  }
  pre {
    max-height: 320px;
    overflow: auto;
    margin-top: var(--space-3);
    font-size: 11px;
  }
</style>
