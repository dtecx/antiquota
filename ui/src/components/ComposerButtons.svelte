<!-- Presents goal, stop and send actions using the current composer state. -->
<script lang="ts">
  import type { Workbench } from '../lib/workbench';
  import Icon from './Icon.svelte';
  let {
    workbench,
    running,
    connected,
    text,
    ready,
    local,
    openGoal,
  }: {
    workbench: Workbench;
    running: boolean;
    connected: boolean;
    text: string;
    ready: string[];
    local: string;
    openGoal: () => void;
  } = $props();
  const view = $derived(workbench.state);
</script>

<div class="actions">
  <button
    type="button"
    class="goal-button"
    class:active={$view.selected?.goal?.state === 'active'}
    title="Work on an objective until it is verified done"
    onclick={openGoal}
    disabled={!$view.selected}>◎ Goal</button
  >{#if running}<button
      type="button"
      class="stop"
      onclick={() => workbench.stop()}
      disabled={$view.mutating || !connected}><Icon name="stop" />Stop</button
    >{/if}<button
    type="submit"
    class="primary send"
    disabled={(!text.trim() && !ready.length) ||
      (local !== '' && text.trim().length < 2) ||
      $view.mutating ||
      !connected ||
      $view.loading}
    ><span
      >{local === 'shell' ? 'Run' : local === 'note' ? 'Save' : running ? 'Queue' : 'Send'}</span
    ><Icon name="send" /></button
  >
</div>

<style>
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
  .goal-button {
    min-height: 32px;
    padding: var(--space-1) var(--space-3);
    font-size: 12px;
    color: var(--ink-2);
  }
  .goal-button.active {
    color: var(--baton);
    border-color: var(--baton);
  }
</style>
