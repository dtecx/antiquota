<!-- Owns the bars above the composer: the session's goal and a finished plan. -->
<script lang="ts">
  import type { Workbench } from '../lib/workbench';
  import { draftFor, goalDialog, goalMeta, planReady } from '../lib/goals';
  let { workbench, onrevise }: { workbench: Workbench; onrevise: () => void } = $props();
  const view = $derived(workbench.state);
  const session = $derived($view.selected);
  const goal = $derived(session?.goal ?? null);
  const plan = $derived(planReady(session, $view.events));
  function edit() {
    if (!session) return;
    goalDialog.set(
      draftFor(
        goal,
        $view.drafts[session.id] ?? '',
        $view.snapshot?.defaults.verifier_model || session.model,
      ),
    );
  }
</script>

<div class="bars">
  {#if session && goal}
    <section class="bar goal" class:met={goal.state === 'met'} aria-label="Goal">
      <span class="label">◎ goal</span>
      <div class="body">
        <p class="objective" title={goal.objective}>{goal.objective}</p>
        <p class="meta">{goalMeta(goal, session.model)}</p>
      </div>
      <div class="actions">
        {#if goal.state === 'active'}<button
            class="small"
            disabled={$view.mutating}
            onclick={() => workbench.stop()}>Pause</button
          >{:else if goal.state !== 'met'}<button
            class="small"
            disabled={$view.mutating}
            onclick={() => workbench.resumeGoal()}>Resume</button
          >{/if}<button class="small" onclick={edit}>Edit</button><button
          class="small quiet"
          aria-label="Remove goal"
          title="Remove goal"
          disabled={$view.mutating}
          onclick={() => workbench.clearGoal()}>✕</button
        >
      </div>
    </section>
  {/if}
  {#if plan}
    <section class="bar plan" aria-label="Plan ready">
      <span class="label">▤ plan ready</span>
      <div class="body">
        <p class="meta">agy only planned (plan mode: nothing was changed).</p>
      </div>
      <div class="actions">
        <button
          class="small primary"
          disabled={$view.mutating}
          onclick={() => workbench.executePlan('accept-edits')}>Execute plan</button
        ><button
          class="small"
          title="Edits and unknown commands wait for your approval"
          disabled={$view.mutating}
          onclick={() => workbench.executePlan('default')}>Execute with approvals</button
        ><button class="small" onclick={onrevise}>Revise</button>
      </div>
    </section>
  {/if}
</div>

<style>
  .bars {
    width: 100%;
    max-width: var(--transcript-width);
    margin: 0 auto;
    padding: 0 var(--space-5);
  }
  .bars:empty {
    display: none;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--rule-2);
    border-radius: var(--r-md);
    background: var(--night-1);
  }
  .goal {
    border-color: var(--baton);
    background: var(--baton-soft);
  }
  .goal.met {
    border-color: var(--ok);
    background: var(--ok-soft);
  }
  .label {
    flex-shrink: 0;
    color: var(--baton);
    font: 500 10px var(--mono);
    letter-spacing: 0.06em;
  }
  .met .label {
    color: var(--ok);
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .objective {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--ink);
  }
  .meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-3);
    font: 10px var(--mono);
  }
  .actions {
    display: flex;
    flex-shrink: 0;
    gap: var(--space-2);
  }
  .small {
    font-size: 11px;
    min-height: 26px;
    padding: var(--space-1) var(--space-2);
  }
  @media (max-width: 760px) {
    .bars {
      padding: 0 var(--space-3);
    }
    .bar {
      flex-wrap: wrap;
    }
  }
</style>
