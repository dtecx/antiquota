<!-- Owns one account quota pool and the reset explanations for its windows. -->
<script lang="ts">
  import type { Account } from '../gen/Account';
  import {
    clockTime,
    quotaPercent,
    quotaWidth,
    resetDescription,
    wallReason,
  } from '../lib/account-format';
  let { account, pool, label }: { account: Account; pool: string; label: string } = $props();
</script>

<section class="quota-pool" aria-label={`${label} quota for ${account.slot}`}>
  <div class="pool-heading">
    <h4>{label}</h4>
    {#if account.walls[pool]}<span class="wall" title={account.why[pool] ?? ''}
        >{wallReason(account.why[pool])} until {clockTime(account.walls[pool])}</span
      >{/if}
  </div>
  {#each Object.entries(account.quota[pool] ?? {}) as [window, [fraction, reset]] (window)}
    <div class="window" title={resetDescription(reset)}>
      <span>{window === '5h' ? '5 hour' : window}</span>
      <div
        class="bar"
        class:wall={fraction <= 0.02}
        class:low={fraction > 0.02 && fraction < 0.2}
        role="meter"
        aria-label={`${label} ${window} remaining`}
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow={Math.round(fraction * 100)}
      >
        <i style:width={`${quotaWidth(fraction)}%`}></i>
      </div>
      <span class="percentage number">{quotaPercent(fraction)}</span>
    </div>
  {:else}<p class="muted">Quota not read yet.</p>{/each}
</section>

<style>
  .quota-pool {
    display: grid;
    gap: var(--space-2);
  }
  .pool-heading {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-2);
    justify-content: space-between;
  }
  h4 {
    margin: 0;
    color: var(--ink-2);
    font-size: 11px;
    font-weight: 500;
  }
  .wall {
    color: var(--bad);
    font-size: 10px;
  }
  .window {
    display: grid;
    grid-template-columns: 56px minmax(40px, 1fr) 40px;
    align-items: center;
    gap: var(--space-2);
    font-size: 11px;
    color: var(--ink-3);
  }
  .bar {
    height: 4px;
    background: var(--rule-2);
    border-radius: var(--r-sm);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--ok);
  }
  .bar.low i {
    background: var(--baton);
  }
  .bar.wall i {
    background: var(--bad);
  }
  .percentage {
    font: 10px var(--mono);
    text-align: right;
    color: var(--ink-2);
  }
  p {
    font-size: 11px;
  }
</style>
