<!-- Owns one tool approval: the waiting request with its choices, or the answered receipt. -->
<script lang="ts">
  import {
    type ApprovalEvent,
    alwaysChoice,
    answerSummary,
    approvalKind,
    approvalPreview,
    approvalTarget,
    approvalTitle,
    waitMinutes,
  } from '../lib/approvals';
  let {
    event,
    cwd,
    onanswer,
  }: {
    event: ApprovalEvent;
    cwd: string;
    onanswer: (id: string, decision: 'allow' | 'deny', always: boolean) => Promise<void>;
  } = $props();
  let sending = $state(false);
  const target = $derived(approvalTarget(event, cwd));
  const preview = $derived(approvalPreview(event));
  const always = $derived(alwaysChoice(event));
  async function answer(decision: 'allow' | 'deny', permanent: boolean) {
    sending = true;
    try {
      await onanswer(event.id, decision, permanent);
    } finally {
      sending = false;
    }
  }
</script>

{#if event.state === 'pending'}
  <section class="approval" role="group" aria-label="Approval needed">
    <header>
      <span class="label">Approval needed</span>
      <span class="title">{approvalTitle(event, cwd)}</span>
      <span class="wait">waits {waitMinutes(event)} min</span>
    </header>
    {#if preview}<pre class:command={approvalKind(event) === 'command'}>{preview}</pre>{/if}
    <div class="actions">
      <button class="primary" disabled={sending} onclick={() => answer('allow', false)}
        >Allow once</button
      >{#if always}<button
          disabled={sending}
          title={always.title}
          onclick={() => answer('allow', true)}>{always.label}</button
        >{/if}<button class="quiet" disabled={sending} onclick={() => answer('deny', false)}
        >Decline</button
      >
    </div>
  </section>
{:else}
  <p class="receipt" class:declined={event.state !== 'allowed'}>
    <span class="mark">{event.state === 'allowed' ? '✓ allowed' : '✕ declined'}</span>
    <code title={target}>{target}</code>
    <span class="detail">{answerSummary(event)}</span>
  </p>
{/if}

<style>
  .approval {
    margin: var(--space-4) 0;
    border: 1px solid var(--baton);
    border-radius: var(--r-md);
    background: var(--baton-soft);
    padding: var(--space-4);
  }
  header {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    flex-wrap: wrap;
  }
  .label {
    color: var(--baton);
    font: 500 10px var(--mono);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .title {
    color: var(--ink);
    font-size: 13px;
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .wait {
    margin-left: auto;
    color: var(--ink-3);
    font: 10px var(--mono);
  }
  pre {
    margin: var(--space-3) 0 0;
    max-height: 240px;
    overflow: auto;
    background: var(--well);
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
    padding: var(--space-3);
    color: var(--ink-2);
    font-size: 11px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  pre.command {
    color: var(--ink);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }
  .actions button {
    font-size: 12px;
  }
  .receipt {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    margin: var(--space-2) 0;
    font: 11px var(--mono);
    color: var(--ink-3);
    min-width: 0;
  }
  .mark {
    color: var(--ok);
    flex-shrink: 0;
  }
  .declined .mark {
    color: var(--bad);
  }
  code {
    color: var(--ink-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .detail {
    margin-left: auto;
    flex-shrink: 0;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
