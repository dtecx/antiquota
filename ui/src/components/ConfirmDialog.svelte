<!-- Owns the in-app confirmation dialog for destructive actions. -->
<script lang="ts">
  import { question } from '../lib/confirm';
  let dialog: HTMLDialogElement;
  $effect(() => {
    if ($question) {
      if (!dialog?.open) dialog?.showModal();
    } else dialog?.close();
  });
</script>

<dialog
  bind:this={dialog}
  oncancel={(event) => {
    event.preventDefault();
    $question?.answer(false);
  }}
  aria-labelledby="confirm-title"
>
  {#if $question}
    <h2 id="confirm-title">{$question.title}</h2>
    <p>{$question.text}</p>
    <div class="actions">
      <button class="quiet" onclick={() => $question?.answer(false)}>Cancel</button><button
        class:danger={$question.danger}
        class:primary={!$question.danger}
        onclick={() => $question?.answer(true)}>{$question.ok}</button
      >
    </div>
  {/if}
</dialog>

<style>
  dialog {
    width: min(400px, calc(100vw - 32px));
    padding: var(--space-5);
    border: 1px solid var(--rule-3);
    border-radius: var(--r-md);
    background: var(--night-1);
    color: var(--ink);
    box-shadow: 0 24px 80px rgb(0 0 0 / 40%);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 55%);
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  p {
    margin: var(--space-3) 0 var(--space-5);
    color: var(--ink-2);
    font-size: 12px;
    line-height: 1.7;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
  .danger {
    background: var(--bad);
    border-color: var(--bad);
    color: var(--night);
  }
</style>
