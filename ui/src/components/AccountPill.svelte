<!-- Owns the compact sidebar quota strip and serving-account summary. -->
<script lang="ts">
  import type { Accounts } from '../lib/accounts';
  import type { Login } from '../lib/login';
  import type { Session } from '../gen/Session';
  import { accountSummary, accountStrip, modelPool } from '../lib/account-format';
  let { accounts, login, session }: { accounts: Accounts; login: Login; session: Session | null } =
    $props();
  const view = $derived(accounts.state);
  const pool = $derived(modelPool(session?.model ?? 'gemini-3.8-flash-high'));
  const summary = $derived(accountSummary($view.accounts, session?.last_slot ?? null, pool));
</script>

<button
  class="pill quiet"
  aria-label="Accounts and quota"
  onclick={() => ($view.accounts.length ? accounts.open() : login.open())}
>
  <span class="strip" aria-label="Account quota status"
    >{#each $view.accounts as account (account.slot)}{@const strip = accountStrip(account, pool)}<i
        class={strip.state}
        style:opacity={strip.fraction}
        title={account.slot}
      ></i>{/each}</span
  >
  <span class="main"
    >{summary.main}{#if summary.attention}<span class="attention" title="Accounts need attention"
        >●</span
      >{/if}</span
  ><span class="detail">{summary.detail}</span>
</button>

<style>
  .pill {
    width: 100%;
    text-align: left;
    padding: var(--space-3) var(--space-4);
    display: grid;
    gap: var(--space-1);
    border-top: 1px solid var(--rule);
    border-radius: 0;
  }
  .strip {
    display: flex;
    gap: var(--space-1);
    height: 3px;
    margin-bottom: var(--space-2);
  }
  .strip i {
    flex: 1;
    background: var(--ok);
    border-radius: var(--r-sm);
  }
  .strip i.wall {
    background: var(--bad);
  }
  .strip i.low {
    background: var(--baton);
  }
  .strip i.off {
    background: var(--ink-4);
  }
  .main {
    font-size: 11px;
    color: var(--ink-2);
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .detail {
    font-size: 10px;
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .attention {
    color: var(--baton);
    font-size: 8px;
  }
</style>
