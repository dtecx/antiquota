<!-- Owns the compact newest-first account decision history. -->
<script lang="ts">
  import type { RoutingEntry } from '../gen/RoutingEntry';
  import type { Session } from '../gen/Session';
  import { routingLine } from '../lib/routing-format';
  import { clockTime } from '../lib/account-format';
  let { entries, sessions }: { entries: RoutingEntry[]; sessions: Session[] } = $props();
</script>

<section class="routing" aria-label="Routing log">
  <h3>Routing log</h3>
  <p>Which account served what, walls, cooldowns and retries, newest first.</p>
  <div class="rows">
    {#each entries.slice().reverse() as entry, index (`${entry.ts}-${index}`)}
      {@const line = routingLine(entry, sessions)}
      <div
        class="row"
        class:pick={entry.event === 'pick'}
        class:problem={['wall', 'signed-out'].includes(entry.event)}
        title={entry.why ?? ''}
      >
        <time>{clockTime(entry.ts)}</time><span class="kind">{line.label}</span><span
          >{line.text}</span
        >
      </div>
    {:else}<p>No routing decisions yet.</p>{/each}
  </div>
</section>

<style>
  .routing {
    display: grid;
    gap: var(--space-3);
  }
  h3 {
    font-size: 12px;
    font-weight: 600;
  }
  p {
    font-size: 11px;
    color: var(--ink-3);
  }
  .rows {
    max-height: 240px;
    overflow: auto;
  }
  .row {
    display: grid;
    grid-template-columns: 44px 116px minmax(0, 1fr);
    gap: var(--space-2);
    align-items: baseline;
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--rule);
    font-size: 11px;
    color: var(--ink-2);
    overflow-wrap: anywhere;
  }
  time {
    font: 10px var(--mono);
    color: var(--ink-4);
  }
  .kind {
    color: var(--ink-3);
  }
  .pick .kind {
    color: var(--baton);
  }
  .problem .kind {
    color: var(--bad);
  }
  @media (max-width: 600px) {
    .row {
      grid-template-columns: 44px minmax(0, 1fr);
    }
    .row > span:last-child {
      grid-column: 2;
    }
  }
</style>
