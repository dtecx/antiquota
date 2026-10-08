<!-- Shows ledger magnitude over time; chart instances remain outside Svelte proxies. -->
<script lang="ts">
  import { Chart, BarController, BarElement, CategoryScale, LinearScale } from 'chart.js';
  import type { UsageResponse } from '../gen/UsageResponse';
  import { createUsage, usageSummary, dayAccounts, tooltipLeft } from '../lib/usage';
  import type { Api } from '../lib/api';
  import { formatTokens } from '../lib/format';
  import { onMount, untrack } from 'svelte';
  let { api }: { api: Api } = $props();
  const usage = untrack(() => createUsage(api)),
    view = usage.state;
  const summary = $derived($view.data ? usageSummary($view.data) : null);
  const period = $derived($view.data?.days.length ?? $view.days);
  let canvas = $state.raw<HTMLCanvasElement>();
  let plot = $state.raw<HTMLDivElement>();
  let chart = $state.raw<Chart<'bar'> | null>(null);
  let hovered = $state<{ day: UsageResponse['days'][number]; left: number } | null>(null);
  Chart.register(BarController, BarElement, CategoryScale, LinearScale);
  onMount(() => {
    void usage.load();
    return usage.dispose;
  });
  $effect(() => {
    const data = $view.data;
    if (!canvas || !data) return;
    const colors = getComputedStyle(canvas),
      ink = colors.getPropertyValue('--ink-3').trim(),
      accent = colors.getPropertyValue('--baton').trim();
    chart = new Chart(canvas, {
      type: 'bar',
      data: {
        labels: data.days.map((d) => d.date),
        datasets: [
          {
            data: data.days.map((d) => d.total),
            backgroundColor: data.days.map((_, i) => (i === data.days.length - 1 ? accent : ink)),
            borderRadius: 4,
            borderSkipped: 'bottom',
            categoryPercentage: 0.95,
            barPercentage: 0.9,
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        animation: false,
        events: [],
        plugins: { legend: { display: false } },
        scales: { x: { display: false }, y: { display: false, beginAtZero: true } },
        layout: { padding: 0 },
      },
    });
    hovered = null;
    return () => {
      chart?.destroy();
      chart = null;
    };
  });
  function hover(index: number) {
    const days = $view.data?.days,
      day = days?.[index];
    if (day && days && plot)
      hovered = {
        day,
        left: tooltipLeft(index, days.length, plot.clientWidth, Math.min(240, plot.clientWidth)),
      };
  }
</script>

<section class="usage" aria-label="Usage">
  <header>
    <div>
      <h3>Usage</h3>
      <p>Tokens antiquota spent through agy, from its own ledger.</p>
    </div>
    <div class="periods" role="group" aria-label="Usage period">
      {#each [7, 14, 30] as days (days)}<button
          aria-pressed={$view.days === days}
          onclick={() => usage.load(days)}>{days} days</button
        >{/each}
    </div>
  </header>
  {#if $view.error}<p role="alert">{$view.error}</p>
    <button onclick={() => usage.load($view.days)}>Retry usage</button>{/if}
  {#if $view.loading}<p role="status">Reading usage…</p>{/if}
  {#if summary && $view.data}
    <dl class="stats">
      <div>
        <dt>{period} days</dt>
        <dd>{formatTokens(summary.total)} <small>{summary.turns} turns</small></dd>
      </div>
      <div>
        <dt>Today</dt>
        <dd>{formatTokens(summary.today.total)} <small>{summary.today.turns} turns</small></dd>
      </div>
      <div>
        <dt>Peak day</dt>
        <dd>{formatTokens(summary.peak)}</dd>
      </div>
      <div>
        <dt>Per turn</dt>
        <dd>
          {summary.turns ? formatTokens(Math.round(summary.mean)) : '–'} <small>average</small>
        </dd>
      </div>
    </dl>
    <div class="plot" bind:this={plot}>
      <canvas
        bind:this={canvas}
        aria-label={`Tokens per day, last ${period} days; total ${formatTokens(summary.total)}`}
      ></canvas>
      <div
        class="day-targets"
        role="group"
        aria-label="Daily usage"
        style:grid-template-columns={`repeat(${$view.data.days.length},1fr)`}
        onmouseleave={() => (hovered = null)}
      >
        {#each $view.data.days as day, index (day.date)}<button
            class="day"
            aria-label={`Usage day ${day.date}: ${day.total} tokens, ${day.turns} turns`}
            onmouseenter={() => hover(index)}
            onfocus={() => hover(index)}
            onblur={() => (hovered = null)}
          ></button>{/each}
      </div>
      {#if hovered}<div class="tip" role="tooltip" style:left={`${hovered.left}px`}>
          <b>{hovered.day.date}</b><span
            >{formatTokens(hovered.day.total)} tokens · {hovered.day.turns} turns</span
          >{#each dayAccounts(hovered.day) as [slot, tokens] (slot)}<span class="contribution"
              ><span>{slot}</span><b>{formatTokens(tokens)}</b></span
            >{/each}
        </div>{/if}
    </div>
    <div class="axis">
      <span>{$view.data.days[0]?.date}</span><span
        >{$view.data.days[Math.floor($view.data.days.length / 2)]?.date}</span
      ><span>today</span>
    </div>
    <div class="tables">
      {#if summary.accounts.length}<table aria-label="Usage by account">
          <thead><tr><th>Account</th><th>{period} days</th><th>Today</th><th>Turns</th></tr></thead
          ><tbody
            >{#each summary.accounts as [slot, row] (slot)}<tr
                ><td title={slot}>{slot}</td><td>{formatTokens(row.tokens)}</td><td
                  >{formatTokens(row.today)}</td
                ><td>{row.turns}</td></tr
              >{/each}</tbody
          >
        </table>{:else}<p>No turns in this period yet.</p>{/if}
      {#if summary.models.length}<table aria-label="Usage by model">
          <thead><tr><th>Model</th><th>Tokens</th><th>Share</th></tr></thead><tbody
            >{#each summary.models as [model, tokens] (model)}<tr
                ><td>{model}</td><td>{formatTokens(tokens)}</td><td
                  >{Math.round((tokens / Math.max(1, summary.total)) * 100)}%</td
                ></tr
              >{/each}</tbody
          >
        </table>{/if}
    </div>
  {/if}
</section>

<style>
  .usage {
    display: grid;
    gap: var(--space-4);
    min-width: 0;
  }
  header {
    display: flex;
    justify-content: space-between;
    gap: var(--space-4);
    flex-wrap: wrap;
  }
  h3 {
    font-size: 17px;
    font-weight: 500;
  }
  p {
    font-size: 12px;
    color: var(--ink-3);
  }
  header p {
    margin-top: var(--space-2);
  }
  .periods {
    display: flex;
    gap: var(--space-1);
  }
  button {
    font-size: 11px;
  }
  button[aria-pressed='true'] {
    color: var(--baton);
    border-color: var(--baton);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-3);
  }
  dt {
    font: 10px var(--mono);
    color: var(--ink-3);
  }
  dd {
    font: 20px var(--mono);
    margin-top: var(--space-2);
  }
  small {
    display: block;
    font: 10px var(--mono);
    color: var(--ink-4);
    margin-top: var(--space-1);
  }
  .plot {
    height: 160px;
    position: relative;
  }
  .day-targets {
    position: absolute;
    inset: 0;
    display: grid;
  }
  .day {
    background: transparent;
    border: 0;
    border-radius: 0;
    min-width: 0;
    padding: 0;
  }
  .day:hover {
    background: var(--rule);
  }
  .day:focus-visible {
    outline: 1px solid var(--baton);
    outline-offset: -1px;
  }
  .tip {
    position: absolute;
    top: var(--space-2);
    width: min(240px, 100%);
    padding: var(--space-3);
    background: var(--night-2);
    border: 1px solid var(--rule-3);
    border-radius: var(--r-sm);
    pointer-events: none;
    display: grid;
    gap: var(--space-2);
    font-size: 11px;
    z-index: 1;
    overflow-wrap: anywhere;
  }
  .contribution {
    display: flex;
    justify-content: space-between;
    gap: var(--space-2);
  }
  .axis {
    display: flex;
    justify-content: space-between;
    font: 10px var(--mono);
    color: var(--ink-4);
  }
  .tables {
    display: grid;
    gap: var(--space-5);
    grid-template-columns: 1fr 1fr;
    min-width: 0;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
    font-size: 11px;
  }
  th {
    text-align: left;
    color: var(--ink-3);
    font-weight: 500;
  }
  td,
  th {
    padding: var(--space-2) var(--space-1);
    border-bottom: 1px solid var(--rule);
    overflow-wrap: anywhere;
  }
  th:first-child {
    width: 45%;
  }
  td:not(:first-child),
  th:not(:first-child) {
    text-align: right;
    font-family: var(--mono);
  }
  @media (max-width: 719px) {
    .stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (max-width: 760px) {
    .tables {
      grid-template-columns: 1fr;
    }
  }
</style>
