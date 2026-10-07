<!-- Owns scroll behavior and empty/loading presentation for the event projection. -->
<script lang="ts">
  import type { Login } from '../lib/login';
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import { groupTranscript } from '../lib/transcript';
  import TurnGroup from './TurnGroup.svelte';
  import { isBusy } from '../lib/format';
  let { workbench, login, api }: { workbench: Workbench; login: Login; api: Api } = $props();
  const view = $derived(workbench.state);
  const groups = $derived(groupTranscript($view.events));
  let scroller: HTMLDivElement;
  let follow = true;
  $effect(() => {
    void $view.events;
    void $view.selected?.id;
    if (follow && scroller)
      requestAnimationFrame(() => {
        scroller.scrollTop = scroller.scrollHeight;
      });
  });
</script>

<div
  class="transcript"
  bind:this={scroller}
  onscroll={() => {
    follow = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 100;
  }}
  aria-label="Conversation"
  aria-busy={$view.loading}
>
  <div class="content">
    {#if $view.loading && !groups.length}<div class="empty">
        <span class="eyebrow">SESSION</span>
        <h2>Loading conversation…</h2>
        <p>Reading the session history.</p>
      </div>
    {:else if !groups.length}<div class="empty">
        <span class="eyebrow">READY WHEN YOU ARE</span>
        <h2>What are we working on?</h2>
        <p>
          Ask a question, inspect a project, or describe a change.<br />Each turn stays with this
          session.
        </p>
        <div class="folder">{$view.selected?.cwd}</div>
        {#if !$view.snapshot?.accounts.length}<button onclick={() => login.open()}
            >Add a Google account</button
          >{/if}
      </div>
    {:else}{#each groups as group, index (group.key)}<TurnGroup
          {group}
          running={index === groups.length - 1 && isBusy($view.selected?.status)}
          cwd={$view.selected?.cwd ?? ''}
          onanswer={workbench.answer}
          onlog={workbench.openJobLog}
          attachmentUrl={(name) => api.attachmentUrl($view.selected?.id ?? '', name)}
        />{/each}{/if}
  </div>
</div>

<style>
  .transcript {
    flex: 1;
    min-height: 0;
    overflow: auto;
    scroll-behavior: auto;
  }
  .content {
    max-width: var(--transcript-width);
    padding: var(--space-5) var(--space-5) 0;
    margin: 0 auto;
  }
  .empty {
    padding: min(14vh, 100px) 0 var(--space-6);
  }
  .eyebrow {
    color: var(--baton);
    font: 10px var(--mono);
    letter-spacing: 0.08em;
  }
  h2 {
    margin: var(--space-3) 0;
    font-size: 24px;
    line-height: 1.3;
    font-weight: 500;
    letter-spacing: -0.5px;
  }
  p {
    color: var(--ink-3);
    font-size: 13px;
    line-height: 1.8;
  }
  .folder {
    margin-top: var(--space-5);
    font: 11px var(--mono);
    color: var(--ink-4);
    overflow-wrap: anywhere;
  }
  @media (max-width: 760px) {
    .content {
      padding: var(--space-4) var(--space-3) 0;
    }
    h2 {
      font-size: 21px;
    }
  }
</style>
