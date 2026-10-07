<!-- Owns the bars for running background jobs and the session's loop or daily schedule. -->
<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { Workbench } from '../lib/workbench';
  import { asLoop, jobDetail, loopDetail, runningJobs } from '../lib/jobs';
  let { workbench }: { workbench: Workbench } = $props();
  const view = $derived(workbench.state);
  const jobs = $derived(runningJobs($view.selected));
  const loop = $derived(asLoop($view.selected?.loop));
  let now = $state(Date.now() / 1000);
  const timer = setInterval(() => (now = Date.now() / 1000), 1000);
  onDestroy(() => clearInterval(timer));
</script>

{#if jobs.length || loop}
  <div class="bars">
    {#each jobs as job (job.id)}
      <section class="bar" aria-label={`Background job ${job.cmd}`}>
        <span class="pulse" aria-hidden="true"></span>
        <code title={job.cmd}>{job.cmd}</code>
        <span class="detail">{jobDetail(job, now)}</span>
        <button class="small" onclick={() => workbench.openJobLog(job.id)}>Log</button><button
          class="small"
          onclick={() => workbench.stopJob(job.id)}>Stop</button
        >
      </section>
    {/each}
    {#if loop}
      <section class="bar" aria-label={loop.at ? 'Daily schedule' : 'Loop'}>
        <span class="glyph" aria-hidden="true">↻</span>
        <code title={loop.prompt}>{loop.prompt}</code>
        <span class="detail">{loopDetail(loop)}</span>
        <button class="small" onclick={() => workbench.deleteLoop()}
          >{loop.at ? 'Remove' : 'Stop loop'}</button
        >
      </section>
    {/if}
  </div>
{/if}

<style>
  .bars {
    width: 100%;
    max-width: var(--transcript-width);
    margin: 0 auto;
    padding: 0 var(--space-5);
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
    font-size: 11px;
  }
  .pulse {
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--baton);
    animation: pulse 1.6s var(--ease) infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .pulse {
      animation: none;
    }
  }
  .glyph {
    color: var(--baton);
  }
  code {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
  }
  .detail {
    flex-shrink: 0;
    color: var(--ink-3);
    font: 10px var(--mono);
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
