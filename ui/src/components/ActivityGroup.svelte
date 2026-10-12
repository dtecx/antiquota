<!-- Folds one chronological run of adjacent tool calls without moving intervening events. -->
<script lang="ts">
  import { preferences } from '../lib/preferences';
  import { activitySummary, type ToolEvent } from '../lib/tools';
  import type { Expansion } from '../lib/transcript-actions';
  import ToolStep from './ToolStep.svelte';
  let { events, cwd, expand }: { events: ToolEvent[]; cwd: string; expand: Expansion } = $props();
  const prefs = preferences.state;
  const summary = $derived(activitySummary(events, cwd));
  let open = $state(false);
  $effect(() => {
    const intent = expand;
    if (intent.version > 0) open = intent.open;
  });
</script>

{#snippet rows()}
  {#each events as event (`${event.turn}-${event.step}`)}<ToolStep {event} {cwd} {expand} />{/each}
{/snippet}
{#if $prefs.view === 'steps' || $prefs.detail === 'full'}{@render rows()}
{:else}<details class="activity" class:running={!!summary.running} bind:open>
    <summary
      ><span class="label"
        >{#if $prefs.detail !== 'compact'}Agent activity{/if}</span
      ><span>{summary.label}</span>
      {#if summary.add}<span class="added">+{summary.add}</span>{/if}{#if summary.del}<span
          class="deleted">−{summary.del}</span
        >{/if}
      {#if summary.failed}<span class="failed">{summary.failed} failed</span>{/if}
      <span class="steps">{summary.steps} {summary.steps === 1 ? 'step' : 'steps'}</span>
    </summary>
    <div class="activity-body">{@render rows()}</div>
  </details>{/if}

<style>
  .activity {
    margin: 0;
  }
  summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
    list-style: none;
    min-height: 28px;
    font-size: 12px;
    color: var(--ink-3);
  }
  summary::-webkit-details-marker {
    display: none;
  }
  .label {
    color: var(--ink-4);
    font: 10px var(--mono);
  }
  .label::before {
    content: '';
    display: inline-block;
    width: 12px;
    height: 1px;
    background: var(--baton);
    margin-right: var(--space-2);
    vertical-align: middle;
  }
  .running summary {
    color: var(--baton);
  }
  .added {
    color: var(--ok);
  }
  .deleted,
  .failed {
    color: var(--bad);
  }
  .steps {
    color: var(--ink-4);
    margin-left: auto;
    font: 10px var(--mono);
  }
  .activity-body {
    padding: var(--space-2) var(--space-3);
    border-left: 1px solid var(--rule-2);
    margin-left: 5px;
  }
</style>
