<!-- Shows the current session state below the composer. -->
<script lang="ts">
  import type { Workbench } from '../lib/workbench';
  import { formatTokens, inputTokens, isBusy, statusLabel } from '../lib/format';
  let { workbench }: { workbench: Workbench } = $props();
  const view = $derived(workbench.state);
  const running = $derived(isBusy($view.selected?.status));
  const attention = $derived($view.selected?.status === 'approval');
</script>

<div class="session-state">
  <span class="status" class:running class:attention
    ><span class="dot"></span>{statusLabel($view.selected?.status ?? 'idle')}</span
  >{#if $view.selected?.queue.length}<span>{$view.selected.queue.length} queued</span>{/if}<span
    class="tokens"
    >{formatTokens($view.selected ? inputTokens($view.selected.usage) : 0)} in <span>/</span>
    {formatTokens($view.selected?.usage.output ?? 0)} out</span
  >
</div>

<style>
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
</style>
