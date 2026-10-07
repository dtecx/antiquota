<!-- Owns the consent/code form; all lifecycle state and timers live in the login store. -->
<script lang="ts">
  import type { Login } from '../lib/login';
  import type { Accounts } from '../lib/accounts';
  import VerificationBlock from './VerificationBlock.svelte';
  import Icon from './Icon.svelte';
  let { login, accounts }: { login: Login; accounts: Accounts } = $props();
  const view = $derived(login.state);
  const accountView = $derived(accounts.state);
  const waiting = $derived($view.phase === 'waiting');
  let dialog: HTMLDialogElement;
  let codeInput: HTMLInputElement;
  $effect(() => {
    if (waiting) codeInput?.focus();
  });
  $effect(() => {
    if ($view.open) dialog?.showModal();
    else dialog?.close();
  });
</script>

<svelte:window onfocus={() => login.paste(true)} />
<dialog bind:this={dialog} aria-labelledby="login-title" oncancel={() => login.close()}>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      void login.submit();
    }}
  >
    <header>
      <h2 id="login-title">Add Google account</h2>
      <button
        type="button"
        class="icon quiet"
        aria-label="Close sign-in"
        onclick={() => login.close()}><Icon name="close" /></button
      >
    </header>
    <p>agy signs this account into antiquota’s own slot. Your other accounts stay signed in.</p>
    <button
      class="primary consent"
      type="button"
      disabled={['starting', 'submitting'].includes($view.phase)}
      onclick={() => login.start()}
      >{$view.phase === 'starting'
        ? 'Starting sign-in…'
        : $view.url
          ? 'New sign-in link'
          : 'Open Google sign-in'}</button
    >
    {#if $view.message}<p class:success={$view.phase === 'done'} role="status">
        {$view.message}
      </p>{/if}
    {#if $view.url && ['waiting', 'submitting'].includes($view.phase)}<a
        href={$view.url}
        target="_blank"
        rel="noopener noreferrer"
        >{$view.message.startsWith('Open') ? 'Google sign-in page' : 'Reopen the page'}</a
      >{/if}
    {#if $view.phase === 'verify' && $view.verify}<VerificationBlock
        email={$view.name}
        url={$view.verify}
        checking={$accountView.busy === $view.name}
        check={() => accounts.check($view.name)}
      />{/if}
    {#if $view.phase === 'verify' && $accountView.feedback}<p class="success" role="status">
        {$accountView.feedback}
      </p>{/if}
    {#if $view.phase === 'verify' && $accountView.error}<p class="error" role="alert">
        {$accountView.error}
      </p>{/if}
    <label
      >Sign-in code<input
        bind:this={codeInput}
        aria-label="Sign-in code"
        disabled={!waiting}
        placeholder="Paste the code from Google"
        autocomplete="off"
        spellcheck="false"
        value={$view.code}
        oninput={(event) => login.code(event.currentTarget.value)}
        onpaste={(event) => {
          const code = event.clipboardData?.getData('text');
          if (code) {
            event.preventDefault();
            login.pasted(code);
          }
        }}
      /></label
    >
    {#if $view.url && ['waiting', 'submitting'].includes($view.phase)}<div class="code-tools">
        <button
          type="button"
          class="quiet"
          disabled={!waiting || !!$view.code}
          onclick={() => login.paste()}>Paste from clipboard</button
        ><time class:expiring={$view.seconds <= 15} class="number"
          >{$view.seconds ? `${$view.seconds}s left` : 'link expired'}</time
        >
      </div>{/if}
    {#if $view.error}<p class="error" role="alert">{$view.error}</p>{/if}
    <footer>
      <button type="button" class="quiet" onclick={() => login.close()}>Cancel</button><button
        class="primary"
        type="submit"
        disabled={!waiting || !$view.code.trim()}
        >{$view.phase === 'submitting' ? 'Checking…' : 'Add account'}</button
      >
    </footer>
  </form>
</dialog>

<style>
  dialog {
    width: min(480px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow: auto;
    padding: var(--space-5);
    border: 1px solid var(--rule-3);
    border-radius: var(--r-md);
    color: var(--ink);
    background: var(--night-1);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 55%);
  }
  form {
    display: grid;
    gap: var(--space-3);
  }
  header,
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }
  h2 {
    font-size: 16px;
    font-weight: 600;
  }
  p {
    font-size: 12px;
    color: var(--ink-3);
    line-height: 1.7;
  }
  p.success {
    color: var(--ok);
  }
  p.error {
    color: var(--bad);
  }
  .consent {
    justify-self: start;
  }
  a {
    color: var(--baton);
    font-size: 12px;
  }
  label {
    display: grid;
    gap: var(--space-2);
    font-size: 12px;
    color: var(--ink-2);
  }
  .code-tools {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }
  .code-tools button {
    font-size: 11px;
    padding: var(--space-1);
  }
  time {
    color: var(--ink-3);
    font: 11px var(--mono);
  }
  time.expiring {
    color: var(--bad);
  }
  footer {
    justify-content: flex-end;
    margin-top: var(--space-3);
  }
</style>
