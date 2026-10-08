<!-- Shows the shared command and keyboard reference. -->
<script lang="ts">
  import { commands, shortcuts, shortcutsOpen, shortcutLabel } from '../lib/commands';
  let dialog: HTMLDialogElement;
  $effect(() => {
    if ($shortcutsOpen) {
      if (!dialog?.open) dialog?.showModal();
    } else dialog?.close();
  });
</script>

<dialog
  bind:this={dialog}
  aria-label="Keyboard shortcuts"
  oncancel={() => shortcutsOpen.set(false)}
>
  <h2>Shortcuts and commands</h2>
  <div class="grid">
    <section>
      <h3>Keys</h3>
      <dl>
        {#each shortcuts as [key, label] (key)}<dt><kbd>{shortcutLabel(key ?? '')}</kbd></dt>
          <dd>{label}</dd>{/each}
      </dl>
    </section>
    <section>
      <h3>Commands</h3>
      <dl>
        {#each commands as [command, label] (command)}<dt><code>{command}</code></dt>
          <dd>{label}</dd>{/each}
      </dl>
    </section>
  </div>
  <button class="primary" onclick={() => shortcutsOpen.set(false)}>Close</button>
</dialog>

<style>
  dialog {
    width: min(880px, calc(100vw - 32px));
    max-height: 85vh;
    overflow: auto;
    background: var(--night);
    color: var(--ink);
    border: 1px solid var(--rule-3);
    border-radius: var(--r-md);
    padding: var(--space-5);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 45%);
  }
  h2 {
    font-size: 16px;
    margin-bottom: var(--space-4);
  }
  h3 {
    font-size: 12px;
    margin-bottom: var(--space-3);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-5);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-3);
    font-size: 11px;
  }
  dd {
    color: var(--ink-3);
  }
  dt {
    font: 11px var(--mono);
  }
  button {
    float: right;
    margin-top: var(--space-4);
  }
  @media (max-width: 679px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
