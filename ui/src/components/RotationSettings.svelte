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
  const placements = [
    {
      value: 'fill',
      title: 'Fill first',
      kind: 'sequential · default',
      text: 'Keep new sessions on one account until its quota runs out, then move to the next.',
      // Three sessions on the first account, then the next one.
      sketch: [3, 0],
    },
    {
      value: 'spread',
      title: 'Spread first',
      kind: 'parallel',
      text: 'Put new sessions on accounts without a running session first, so their quotas work at once.',
      sketch: [1, 1, 1],
    },
  ];
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
    <fieldset class="placement" disabled={!$view.defaults}>
      <legend>New sessions</legend>
      {#each placements as placement (placement.value)}<label
          class="choice"
          class:chosen={$view.fields.placement === placement.value}
          ><input
            type="radio"
            name="placement"
            value={placement.value}
            checked={$view.fields.placement === placement.value}
            onchange={() => {
              accounts.field('placement', placement.value);
              void accounts.saveField('placement');
            }}
          /><span class="title">{placement.title}<small>{placement.kind}</small></span><span
            class="text">{placement.text}</span
          ><span class="sketch" aria-hidden="true"
            >{#each placement.sketch as sessions, index (index)}<span class="account"
                >{String.fromCharCode(65 + index)}<span class="dots"
                  >{'●'.repeat(sessions) || '○'}</span
                ></span
              >{/each}</span
          ></label
        >{/each}
    </fieldset>
    <p>
      {captions[$view.fields.rotation] ?? captions.perishable} A session keeps its account while it has
      headroom; several sessions can run on one account at once. A spent pool is walled until its reset;
      a short rate limit only rests the account.
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
  .placement {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-3);
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
  }
  legend {
    font-size: 11px;
    color: var(--ink-2);
    margin-bottom: var(--space-2);
  }
  .choice {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-1) var(--space-2);
    padding: var(--space-3);
    border: 1px solid var(--rule-3);
    border-radius: var(--r-md);
    cursor: pointer;
  }
  .choice.chosen {
    border-color: var(--baton);
  }
  .choice input {
    grid-column: auto;
    margin: 2px 0 0;
  }
  .title {
    font-size: 12px;
    font-weight: 600;
    color: var(--ink);
  }
  .title small {
    font-weight: 400;
    margin-left: var(--space-2);
  }
  .text,
  .sketch {
    grid-column: 2;
  }
  .text {
    color: var(--ink-3);
    line-height: 1.6;
  }
  .sketch {
    display: flex;
    gap: var(--space-2);
    font: 10px var(--mono);
    color: var(--ink-3);
  }
  .account {
    display: inline-flex;
    gap: var(--space-1);
    padding: 2px var(--space-2);
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
  }
  .dots {
    color: var(--baton);
  }
  @media (max-width: 600px) {
    .fields,
    .placement {
      grid-template-columns: 1fr;
    }
  }
</style>
