<!-- Owns the job log viewer: the end of a job's output, scrolled to the latest line. -->
<script lang="ts">
  import { tick } from 'svelte';
  import { jobLog } from '../lib/jobs';
  import Icon from './Icon.svelte';
  let dialog: HTMLDialogElement;
  let body: HTMLPreElement | undefined = $state();
  $effect(() => {
    if ($jobLog) {
      if (!dialog?.open) dialog?.showModal();
      void tick().then(() => {
        if (body) body.scrollTop = body.scrollHeight;
      });
    } else dialog?.close();
  });
</script>

<dialog bind:this={dialog} oncancel={() => jobLog.set(null)} aria-labelledby="job-log-title">
  {#if $jobLog}
    <div class="heading">
      <div>
        <span class="eyebrow">JOB LOG</span>
        <h2 id="job-log-title" title={$jobLog.cmd}>{$jobLog.cmd}</h2>
        <p class="path">{$jobLog.log}</p>
      </div>
      <button class="icon quiet" aria-label="Close job log" onclick={() => jobLog.set(null)}
        ><Icon name="close" /></button
      >
    </div>
    <pre bind:this={body} aria-label="Job output">{$jobLog.text || '(no output yet)'}</pre>
  {/if}
</dialog>

<style>
  dialog {
    width: min(900px, calc(100vw - 32px));
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
  .heading {
    display: flex;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-5);
    border-bottom: 1px solid var(--rule);
  }
  .heading > div {
    min-width: 0;
  }
  .eyebrow {
    color: var(--baton);
    font: 10px var(--mono);
    letter-spacing: 0.08em;
  }
  h2 {
    margin-top: var(--space-1);
    font: 500 13px var(--mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    color: var(--ink-4);
    font: 10px var(--mono);
    overflow-wrap: anywhere;
  }
  pre {
    margin: 0;
    max-height: 60vh;
    overflow: auto;
    padding: var(--space-4) var(--space-5);
    background: var(--well);
    color: var(--ink-2);
    font-size: 11px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
