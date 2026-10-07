<!-- Owns the prompt control and send/stop affordances; drafts live in the store. -->
<script lang="ts">
  import type { Workbench } from '../lib/workbench';
  import Icon from './Icon.svelte';
  import { formatTokens, isBusy, statusLabel } from '../lib/format';
  let { workbench, connected }: { workbench: Workbench; connected: boolean } = $props();
  const view = $derived(workbench.state);
  const running = $derived(isBusy($view.selected?.status));
  const attention = $derived($view.selected?.status === 'approval');
  const text = $derived($view.drafts[$view.selected?.id ?? ''] ?? '');
  function keydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      if (connected && !$view.mutating) void workbench.send();
    }
  }
</script>

<div class="composer-wrap">
  {#if $view.feedback}<p class="feedback" role="status">{$view.feedback}</p>{/if}
  <form
    class="composer"
    onsubmit={(event) => {
      event.preventDefault();
      void workbench.send();
    }}
  >
    <label class="sr-only" for="prompt">Message</label><textarea
      id="prompt"
      aria-label="Message"
      rows="3"
      placeholder={running ? 'Queue a message for the next turn…' : 'Message antiquota…'}
      value={text}
      oninput={(event) => workbench.draft(event.currentTarget.value)}
      onkeydown={keydown}
      disabled={!$view.selected || $view.loading}></textarea>
    <div class="composer-tools">
      <span class="hint"><kbd>↵</kbd> send <span>·</span> <kbd>⇧ ↵</kbd> newline</span>
      <div class="actions">
        {#if running}<button
            type="button"
            class="stop"
            onclick={() => workbench.stop()}
            disabled={$view.mutating || !connected}><Icon name="stop" />Stop</button
          >{/if}<button
          type="submit"
          class="primary send"
          disabled={!text.trim() || $view.mutating || !connected || $view.loading}
          ><span>{running ? 'Queue' : 'Send'}</span><Icon name="send" /></button
        >
      </div>
    </div>
  </form>
  <div class="session-state">
    <span class="status" class:running class:attention
      ><span class="dot"></span>{statusLabel($view.selected?.status ?? 'idle')}</span
    >{#if $view.selected?.queue.length}<span>{$view.selected.queue.length} queued</span>{/if}<span
      class="tokens"
      >{formatTokens($view.selected?.usage.input ?? 0)} in <span>/</span>
      {formatTokens($view.selected?.usage.output ?? 0)} out</span
    >
  </div>
</div>

<style>
  .composer-wrap {
    width: 100%;
    max-width: var(--transcript-width);
    padding: 0 var(--space-5) var(--space-2);
    margin: 0 auto;
  }
  .composer {
    border: 1px solid var(--rule-3);
    border-radius: var(--r-md);
    background: var(--night-1);
    overflow: hidden;
  }
  .composer:focus-within {
    border-color: var(--baton);
  }
  textarea {
    border: 0;
    padding: var(--space-4);
    min-height: 92px;
    max-height: 30vh;
    background: transparent;
    font-size: 13px;
    line-height: 1.7;
  }
  textarea:focus {
    outline: none;
  }
  .composer-tools {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--space-3) var(--space-3);
    gap: var(--space-2);
  }
  .hint {
    color: var(--ink-4);
    font-size: 10px;
  }
  .hint > span {
    padding: 0 var(--space-1);
  }
  .actions {
    display: flex;
    gap: var(--space-2);
    margin-left: auto;
  }
  .send,
  .stop {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 32px;
    padding: var(--space-1) var(--space-3);
    font-size: 12px;
  }
  .stop {
    color: var(--ink-2);
  }
  .session-state {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    min-height: 30px;
    font: 10px var(--mono);
    color: var(--ink-4);
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  .status.running {
    color: var(--baton);
  }
  .status.attention {
    color: var(--baton);
    font-weight: 600;
  }
  .status.attention .dot {
    width: 6px;
    height: 6px;
  }
  .dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: currentColor;
  }
  .tokens {
    margin-left: auto;
  }
  .tokens > span {
    color: var(--ink-4);
  }
  .feedback {
    margin-bottom: var(--space-2);
    color: var(--baton);
    font-size: 11px;
  }
  @media (max-width: 760px) {
    .composer-wrap {
      padding: 0 var(--space-3) var(--space-2);
    }
    .hint {
      display: none;
    }
  }
</style>
