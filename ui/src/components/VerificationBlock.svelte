<!-- Owns the verification explanation and exact identity-specific verification link. -->
<script lang="ts">
  let {
    email,
    url,
    checking = false,
    check,
  }: { email: string; url: string; checking?: boolean; check: () => void } = $props();
</script>

<div class="verification">
  <p>
    Google wants this account verified before Antigravity serves it. Open the link signed in as <strong
      >{email}</strong
    >, finish the steps, then check again. antiquota also re-checks it every 10 minutes.
  </p>
  <div class="actions">
    <a href={url} target="_blank" rel="noopener noreferrer">Verify with Google</a>
    <button disabled={checking} onclick={check}>{checking ? 'Checking…' : 'Check again'}</button>
  </div>
</div>

<style>
  .verification {
    display: grid;
    gap: var(--space-3);
    border-left: 2px solid var(--baton);
    padding-left: var(--space-3);
  }
  p {
    font-size: 12px;
    color: var(--ink-2);
    line-height: 1.7;
    overflow-wrap: anywhere;
  }
  strong {
    color: var(--ink);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-wrap: wrap;
  }
  a {
    color: var(--baton);
    font-size: 11px;
  }
  button {
    font-size: 11px;
  }
</style>
