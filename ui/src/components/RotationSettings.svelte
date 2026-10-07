<!-- Owns labelled rotation controls; drafts and validation belong to the account store. -->
<script lang="ts">
  import type { Accounts } from '../lib/accounts';
  let { accounts }: { accounts: Accounts } = $props();
  const view = $derived(accounts.state);
  const captions: Record<string, string> = {
    perishable:
      'Rotation spends the allowance that resets soonest first, so no quota expires unused.',
    sticky: 'Rotation drains accounts one by one, in order.',
    'round-robin': 'Rotation takes the least recently used account.',
  };
</script>

<section class="rotation" aria-label="Rotation settings">
  <h3>Rotation</h3>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      void accounts.save();
    }}
  >
    {#if $view.saving}<small role="status">Saving rotation…</small>{/if}
    <div class="fields">
      <label
        >Next account<select
          aria-label="Next account"
          disabled={!$view.defaults}
          value={$view.fields.rotation}
          onchange={(event) => {
            accounts.field('rotation', event.currentTarget.value);
            void accounts.saveField('rotation');
          }}
        >
          <option value="perishable">soonest reset first</option><option value="sticky"
            >one by one</option
          ><option value="round-robin">round robin</option>
        </select></label
      >
      <label
        >Reserve <span class="unit">%</span><input
          aria-label="Reserve"
          disabled={!$view.defaults}
          type="number"
          min="0"
          max="50"
          step="any"
          required
          value={$view.fields.reserve}
          oninput={(event) => accounts.field('reserve', event.currentTarget.value)}
          onchange={() => accounts.saveField('reserve')}
        /><small>Leave an account below this</small></label
      >
      <label
        >Quota check <span class="unit">min</span><input
          aria-label="Quota check"
          disabled={!$view.defaults}
          type="number"
          min="2"
          max="240"
          step="any"
          required
          value={$view.fields.quota_poll}
          oninput={(event) => accounts.field('quota_poll', event.currentTarget.value)}
          onchange={() => accounts.saveField('quota_poll')}
        /><small>Idle accounts, no tokens</small></label
      >
    </div>
    <p>
      {captions[$view.fields.rotation] ?? captions.perishable} A session keeps its account while it has
      headroom. A spent pool is walled until its reset; a short rate limit only rests the account.
    </p>
  </form>
</section>

<style>
  .rotation {
    display: grid;
    gap: var(--space-3);
  }
  h3 {
    font-size: 12px;
    font-weight: 600;
  }
  form {
    display: grid;
    gap: var(--space-3);
  }
  .fields {
    display: grid;
    grid-template-columns: 1.4fr 1fr 1fr;
    gap: var(--space-3);
  }
  label {
    display: grid;
    grid-template-columns: 1fr auto;
    align-content: start;
    gap: var(--space-2);
    font-size: 11px;
    color: var(--ink-2);
  }
  input,
  select,
  small {
    grid-column: 1 / -1;
  }
  .unit,
  small {
    color: var(--ink-3);
    font-size: 10px;
  }
  p {
    color: var(--ink-3);
    font-size: 11px;
    line-height: 1.8;
  }
  @media (max-width: 600px) {
    .fields {
      grid-template-columns: 1fr;
    }
  }
</style>
