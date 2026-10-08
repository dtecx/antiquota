<!-- Renders concrete model choices, permission mode and the session meter. -->
<script lang="ts">
  import type { Workbench } from '../lib/workbench';
  import { modelFamilies, pickFamily } from '../lib/models';
  import { modelPool, pools } from '../lib/account-format';
  import { sessionMeter } from '../lib/meter';
  import { modes } from '../lib/composer-actions';
  let {
    workbench,
    changeMode,
  }: { workbench: Workbench; changeMode: (mode: string) => Promise<void> } = $props();
  const view = $derived(workbench.state);
  const session = $derived($view.selected);
  const models = $derived($view.snapshot?.models ?? []);
  const families = $derived(modelFamilies(models, session?.model ?? 'gemini-3.8-flash-high'));
  const current = $derived(families.find((f) => f.variants.some((v) => v.id === session?.model)));
  const meter = $derived(session ? sessionMeter(session, $view.snapshot?.accounts ?? []) : null);
</script>

{#if session}
  <div class="selectors">
    <label
      ><span class="sr-only">Model family</span><select
        aria-label="Model family"
        value={current?.name}
        disabled={$view.mutating}
        onchange={(e) =>
          workbench.update({ model: pickFamily(models, session.model, e.currentTarget.value) })}
      >
        {#each pools as pool (pool.key)}<optgroup label={pool.label}>
            {#each families.filter((f) => modelPool(f.variants[0]?.id ?? '') === pool.key) as family (family.name)}<option
                value={family.name}>{family.name}</option
              >{/each}
          </optgroup>{/each}
      </select></label
    >
    <label
      ><span class="sr-only">Effort</span><select
        aria-label="Effort"
        value={session.model}
        disabled={$view.mutating || (current?.variants.length ?? 0) < 2}
        onchange={(e) => workbench.update({ model: e.currentTarget.value })}
      >
        {#each current?.variants ?? [] as variant (variant.id)}<option value={variant.id}
            >{variant.effort || 'default effort'}</option
          >{/each}
      </select></label
    >
    <label
      ><span class="sr-only">Permission mode</span><select
        aria-label="Permission mode"
        value={session.mode}
        disabled={$view.mutating}
        onchange={(e) => {
          const mode = e.currentTarget.value;
          e.currentTarget.value = session.mode;
          void changeMode(mode);
        }}
      >
        {#each modes as mode (mode)}<option value={mode}>{mode}</option>{/each}
      </select></label
    >
  </div>
  {#if meter}<div class="meter" class:warm={meter.heat === 'warm'} class:hot={meter.heat === 'hot'}>
      <span class="context" title={`${session.context} / ${session.model} context capacity`}
        ><span class="bar"><i style:width={`${meter.fraction * 100}%`}></i></span
        >{meter.context}</span
      >
      <span>{meter.tokens}</span><span class="account">{meter.account}</span><span
        >{meter.available} accounts free</span
      >
    </div>{/if}
{/if}

<style>
  .selectors {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    padding-top: var(--space-2);
  }
  select {
    font: 10px var(--mono);
    padding: var(--space-1) var(--space-2);
    width: auto;
    max-width: 220px;
    background: var(--night-1);
  }
  .meter {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    color: var(--ink-4);
    font: 10px var(--mono);
    padding: var(--space-2) 0;
  }
  .context {
    display: flex;
    gap: var(--space-1);
    align-items: center;
  }
  .bar {
    width: 36px;
    height: 3px;
    background: var(--rule-3);
    display: block;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--ink-3);
  }
  .warm .context {
    color: var(--baton);
  }
  .warm .bar i {
    background: var(--baton);
  }
  .hot .context {
    color: var(--bad);
  }
  .hot .bar i {
    background: var(--bad);
  }
  .account {
    margin-left: auto;
  }
</style>
