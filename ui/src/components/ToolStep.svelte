<!-- Owns one tool call's stable expansion, readable summary and complete output. -->
<script lang="ts">
  import { preferences } from '../lib/preferences';
  import { toolView, type ToolEvent } from '../lib/tools';
  import type { Expansion } from '../lib/transcript-actions';
  import { duration } from '../lib/format';
  const prefs = preferences.state;
  let { event, cwd = '', expand }: { event: ToolEvent; cwd?: string; expand: Expansion } = $props();
  const view = $derived(toolView(event, cwd));
  let open = $state(false);
  $effect(() => {
    const intent = expand;
    if (intent.version > 0) open = intent.open;
  });
</script>

<details class="tool" class:failed={view.state === 'error'} bind:open>
  <summary title={event.name}>
    <span class="glyph" class:active={view.state === 'running'}
      >{view.state === 'running' ? '◌' : view.state === 'error' ? '×' : '✓'}</span
    >
    <span class="name">{view.verb}</span><span class="arg" title={view.arg}>{view.arg}</span>
    {#if view.add}<span class="stats">+{view.add}</span>{/if}{#if view.del}<span class="deleted"
        >−{view.del}</span
      >{/if}
    <span class="state">{view.state}</span>{#if view.secs !== undefined}<span class="time"
        >{duration(view.secs)}</span
      >{/if}
    {#if view.error}<span class="tool-error">{view.error.split('\n').slice(0, 2).join('\n')}</span
      >{/if}
    {#if $prefs.detail === 'detailed' && view.preview}<span
        class="preview"
        class:diff={view.preview.kind === 'diff'}
        >{view.preview.text}{#if view.preview.more}<span class="more"
            >… {view.preview.more} more {view.preview.more === 1 ? 'line' : 'lines'} · click to expand</span
          >{/if}</span
      >{/if}
  </summary>
  <div class="body">
    <pre class:diff={view.body.kind === 'diff'}>{view.body.text}</pre>
    {#if view.body.error}<pre class="error">{view.body.error}</pre>{/if}
  </div>
</details>

<style>
  .tool {
    border-top: 1px solid var(--rule);
  }
  summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
    min-height: 36px;
    font: 11px var(--mono);
    list-style: none;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  .preview,
  .tool-error {
    flex-basis: 100%;
    padding-left: 20px;
    color: var(--ink-3);
    white-space: pre-wrap;
  }
  .glyph {
    color: var(--ok);
    width: 12px;
  }
  .glyph.active {
    color: var(--baton);
  }
  .failed .glyph {
    color: var(--bad);
  }
  .arg {
    color: var(--ink-3);
    overflow-wrap: anywhere;
  }
  .stats {
    color: var(--ok);
  }
  .deleted {
    color: var(--bad);
  }
  .more {
    display: block;
    color: var(--ink-4);
    margin-top: var(--space-1);
  }
  .name {
    color: var(--ink-2);
    overflow-wrap: anywhere;
  }
  .state {
    color: var(--ink-4);
    margin-left: auto;
    font-size: 10px;
  }
  .time {
    color: var(--ink-3);
    font-size: 10px;
  }
  .body {
    margin: 0 0 var(--space-3) 20px;
    background: var(--well);
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
    padding: var(--space-3);
    overflow: hidden;
  }
  pre {
    margin: 0;
    color: var(--ink-2);
    font-size: 11px;
  }
  .tool-error,
  .error {
    color: var(--bad);
  }
</style>
