<!-- Owns one slot's identity, quota, verification and account actions. -->
<script lang="ts">
  import type { Account } from '../gen/Account';
  import type { Accounts } from '../lib/accounts';
  import { pools } from '../lib/account-format';
  import { formatTokens, relativeTime } from '../lib/format';
  import QuotaPool from './QuotaPool.svelte';
  import VerificationBlock from './VerificationBlock.svelte';
  let { account, accounts, signIn }: { account: Account; accounts: Accounts; signIn: () => void } =
    $props();
  const view = $derived(accounts.state);
</script>

<article class="account-card" class:verify={!!account.verify} aria-label={account.slot}>
  <div class="identity">
    <h3>
      {account.slot}{#if account.paused}<span class="paused">paused</span>{/if}
    </h3>
    <span class="probe"
      >{account.busy ? 'running · ' : ''}{account.probed
        ? `probed ${relativeTime(account.probed)}`
        : 'not probed'}</span
    >
  </div>
  {#if account.verify}<VerificationBlock
      email={account.slot}
      url={account.verify}
      checking={$view.busy === account.slot}
      check={() => accounts.check(account.slot)}
    />{/if}
  {#if account.today.turns}<p
      class="today"
      title="Tokens this account used today through antiquota"
    >
      today ↑ {formatTokens(account.today.input ?? 0)} ↓ {formatTokens(
        account.today.output ?? 0,
      )}{#if account.today.thinking}{` ✻ ${formatTokens(account.today.thinking)}`}{/if} · {account
        .today.turns}
      {account.today.turns === 1 ? 'turn' : 'turns'}
    </p>{/if}
  {#if !account.verify}<div class="pools">
      {#each pools as pool (pool.key)}<QuotaPool
          {account}
          pool={pool.key}
          label={pool.label}
        />{/each}
    </div>{/if}
  <div class="actions">
    <button
      disabled={!!$view.busy}
      title={account.paused ? 'Put it back into rotation' : 'Keep it out of rotation'}
      onclick={() => accounts.pause(account)}>{account.paused ? 'Resume' : 'Pause'}</button
    >
    <button
      disabled={!!$view.busy || account.busy}
      title="Read quota and sign-in state now"
      onclick={() => accounts.check(account.slot)}>Check</button
    >
    <button
      disabled={!!$view.busy}
      title="Sign in with the same Google account again"
      onclick={signIn}>Sign in again</button
    >
    <button
      disabled={!!$view.busy || account.busy}
      onclick={() => accounts.requestRemove(account.slot)}>Remove</button
    >
  </div>
</article>

<style>
  .account-card {
    border: 1px solid var(--rule-2);
    border-radius: var(--r-md);
    background: var(--night-1);
    padding: var(--space-4);
    display: grid;
    gap: var(--space-4);
    min-width: 0;
  }
  .account-card.verify {
    border-color: var(--baton-soft);
  }
  .identity {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: var(--space-1) var(--space-3);
    align-items: baseline;
  }
  h3 {
    font-size: 12px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .paused {
    margin-left: var(--space-2);
    font: 10px var(--mono);
    color: var(--ink-3);
    font-weight: 400;
  }
  .probe {
    font-size: 10px;
    color: var(--ink-3);
  }
  .today {
    font: 10px/1.8 var(--mono);
    color: var(--ink-2);
    font-variant-numeric: tabular-nums;
  }
  .pools {
    display: grid;
    gap: var(--space-4);
  }
  .actions {
    display: flex;
    gap: var(--space-1);
    flex-wrap: wrap;
    padding-top: var(--space-2);
    border-top: 1px solid var(--rule);
  }
  .actions button {
    font-size: 11px;
    padding: var(--space-1) var(--space-2);
  }
</style>
