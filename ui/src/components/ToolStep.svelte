<!-- Owns one tool call's state, command parameters, and expandable output. -->
<script lang="ts">
  import { preferences } from '../lib/preferences';
  const prefs = preferences.state;
  import type { EventRecord } from '../gen/EventRecord';
  import { duration } from '../lib/format';
  let { event }: { event: Extract<EventRecord, { t: 'tool' }> } = $props();
  const parameterText = $derived(
    typeof event.params === 'string' ? event.params : JSON.stringify(event.params, null, 2),
  );
</script>

<details class="tool" class:failed={event.state.toLowerCase() === 'error' || Boolean(event.error)}>
  <summary
    ><span class="glyph" class:active={['active', 'running'].includes(event.state.toLowerCase())}
      >{['active', 'running'].includes(event.state.toLowerCase())
        ? '◌'
        : event.error
          ? '×'
          : '✓'}</span
    ><span class="name">{event.name}</span><span class="state">{event.state.toLowerCase()}</span
    >{#if event.secs !== undefined}<span class="time">{duration(event.secs)}</span
      >{/if}{#if $prefs.detail === 'detailed' && event.output}<span class="preview"
        >{event.output.split('\n').slice(-3).join('\n')}</span
      >{/if}</summary
  >
  <div class="body">
    {#if parameterText && parameterText !== 'null'}<pre>{parameterText}</pre>{/if}
    {#if event.output}<div class="output">
        <span>Output</span>
        <pre>{event.output}</pre>
      </div>{/if}
    {#if event.diff}<div class="output">
        <span>Changes</span>
        <pre>{event.diff}</pre>
      </div>{/if}
    {#if event.error}<pre class="error">{typeof event.error === 'string'
          ? event.error
          : JSON.stringify(event.error, null, 2)}</pre>{/if}
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
  .preview {
    flex-basis: 100%;
    padding-left: 20px;
    color: var(--ink-3);
    white-space: pre-wrap;
    max-height: 54px;
    overflow: hidden;
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
  .output {
    border-top: 1px solid var(--rule);
    padding-top: var(--space-3);
    margin-top: var(--space-3);
  }
  .output > span {
    display: block;
    color: var(--ink-4);
    font: 10px var(--mono);
    margin-bottom: var(--space-2);
  }
</style>
