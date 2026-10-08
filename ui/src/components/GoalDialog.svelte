<!-- Owns the goal form: objective, optional check, verifier and round limit. -->
<script lang="ts">
  import { get } from 'svelte/store';
  import type { Workbench } from '../lib/workbench';
  import { goalDialog, goalRequest } from '../lib/goals';
  import { modelFamilies } from '../lib/models';
  import { modelPool, pools } from '../lib/account-format';
  import Icon from './Icon.svelte';
  let { workbench }: { workbench: Workbench } = $props();
  const view = $derived(workbench.state);
  const models = $derived($view.snapshot?.models ?? []);
  let dialog: HTMLDialogElement;
  $effect(() => {
    if ($goalDialog) {
      if (!dialog?.open) dialog?.showModal();
    } else dialog?.close();
  });
  function field(key: 'objective' | 'check' | 'verifier_model' | 'max_rounds', text: string) {
    goalDialog.update((draft) => (draft ? { ...draft, [key]: text, error: '' } : draft));
  }
  async function start() {
    const draft = $goalDialog;
    if (!draft || draft.saving) return;
    if ($view.mutating) {
      goalDialog.set({ ...draft, error: 'Wait for the current action to finish.' });
      return;
    }
    const request = goalRequest(draft);
    if (typeof request === 'string') {
      goalDialog.set({ ...draft, error: request });
      return;
    }
    const owner = workbench;
    const pending = { ...draft, saving: true, error: '' };
    goalDialog.set(pending);
    await owner.setGoal(request);
    if (get(goalDialog) !== pending) return;
    const error = get(owner.state).error;
    if (error) {
      owner.change({ error: '' });
      goalDialog.update((value) => (value ? { ...value, saving: false, error } : value));
    } else goalDialog.set(null);
  }
</script>

<dialog bind:this={dialog} oncancel={() => goalDialog.set(null)} aria-labelledby="goal-title">
  {#if $goalDialog}
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void start();
      }}
    >
      <div class="heading">
        <h2 id="goal-title">Goal</h2>
        <button
          type="button"
          class="icon quiet"
          aria-label="Close goal"
          onclick={() => goalDialog.set(null)}><Icon name="close" /></button
        >
      </div>
      <p class="lead">
        agy keeps working until the objective is met. Each round it either claims completion or is
        pushed on; a check and an independent verifier confirm the claim.
      </p>
      <label
        >Objective<textarea
          aria-label="Objective"
          rows="4"
          placeholder="What should be true when the work is done?"
          value={$goalDialog.objective}
          oninput={(event) => field('objective', event.currentTarget.value)}></textarea></label
      >
      <label
        ><span>Check command <span class="optional">optional</span></span><input
          aria-label="Check command"
          class="mono"
          placeholder="npm test"
          value={$goalDialog.check}
          oninput={(event) => field('check', event.currentTarget.value)}
        /><small>Runs in the project folder; the goal is met only if it exits 0.</small></label
      >
      <label class="toggle"
        ><input
          type="checkbox"
          aria-label="Independent verifier"
          checked={$goalDialog.verifier}
          onchange={(event) =>
            goalDialog.update((draft) =>
              draft ? { ...draft, verifier: event.currentTarget.checked } : draft,
            )}
        />Independent verifier: a fresh read-only agy conversation judges each completion claim</label
      >
      <div class="row">
        <label
          >Verifier model<select
            aria-label="Verifier model"
            disabled={!$goalDialog.verifier}
            value={$goalDialog.verifier_model}
            onchange={(event) => field('verifier_model', event.currentTarget.value)}
          >
            {#each pools as pool (pool.key)}<optgroup label={pool.label}>
                {#each modelFamilies(models, $goalDialog.verifier_model).filter((f) => modelPool(f.variants[0]?.id ?? '') === pool.key) as family (family.name)}
                  {#each family.variants as variant (variant.id)}<option value={variant.id}
                      >{family.name}{variant.effort ? ` · ${variant.effort}` : ''} · {variant.id}</option
                    >{/each}
                {/each}
              </optgroup>{/each}
          </select></label
        >
        <label class="rounds"
          >Rounds<input
            aria-label="Rounds"
            type="number"
            min="1"
            max="500"
            value={$goalDialog.max_rounds}
            oninput={(event) => field('max_rounds', event.currentTarget.value)}
          /></label
        >
      </div>
      {#if $goalDialog.error}<p class="error" role="alert">{$goalDialog.error}</p>{/if}
      <div class="actions">
        <button type="button" class="quiet" onclick={() => goalDialog.set(null)}>Cancel</button
        ><button class="primary" disabled={$goalDialog.saving}>Start goal</button>
      </div>
    </form>
  {/if}
</dialog>

<style>
  dialog {
    width: min(560px, calc(100vw - 32px));
    padding: 0;
    border: 1px solid var(--rule-3);
    border-radius: var(--r-lg);
    color: var(--ink);
    background: var(--night);
    box-shadow: 0 24px 80px rgb(0 0 0 / 40%);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 45%);
  }
  form {
    display: grid;
    gap: var(--space-4);
    padding: var(--space-5);
  }
  .heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    font-size: 16px;
    font-weight: 600;
  }
  .lead {
    color: var(--ink-3);
    font-size: 12px;
    line-height: 1.7;
  }
  label {
    display: grid;
    gap: var(--space-2);
    font-size: 11px;
    color: var(--ink-2);
  }
  .optional,
  small {
    color: var(--ink-4);
    font-size: 10px;
  }
  textarea {
    resize: vertical;
    font-size: 13px;
    line-height: 1.6;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12px;
  }
  .row {
    display: grid;
    grid-template-columns: 1fr 96px;
    gap: var(--space-3);
    align-items: end;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: 12px;
  }
  .toggle input {
    width: auto;
    flex: none;
    margin: 0;
  }
  .error {
    color: var(--bad);
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
  @media (max-width: 520px) {
    .row {
      grid-template-columns: 1fr;
    }
  }
</style>
