<!-- Owns the permission rule lists and approval wait; data and saving belong to the store. -->
<script lang="ts">
  import type { PermissionsStore, RuleKind } from '../lib/permissions';
  import Icon from './Icon.svelte';
  let { permissions }: { permissions: PermissionsStore } = $props();
  const view = $derived(permissions.state);
  let drafts = $state<Record<RuleKind, string>>({ allow: '', deny: '' });
  const lists: { kind: RuleKind; title: string; hint: string }[] = [
    {
      kind: 'allow',
      title: 'Allow without asking',
      hint: 'command(git status) also covers git status --short; risky flags such as find -delete still ask.',
    },
    {
      kind: 'deny',
      title: 'Always refuse',
      hint: 'Deny rules win over allow rules, in every part of a command line.',
    },
  ];
  async function add(kind: RuleKind) {
    if (await permissions.add(kind, drafts[kind])) drafts[kind] = '';
  }
</script>

<section class="permissions" aria-label="Permissions">
  <h3>Permissions</h3>
  <p>
    Shell commands and MCP tools these rules allow run without asking; deny rules always block.
    Everything else waits for your approval in the transcript. The rules apply to every antiquota
    account, not to your agy CLI.
  </p>
  {#if $view.error}<p class="error" role="alert">{$view.error}</p>{/if}
  {#if $view.feedback}<p class="feedback" role="status">{$view.feedback}</p>{/if}
  <div class="lists" aria-busy={$view.loading}>
    {#each lists as list (list.kind)}
      <div class="list">
        <h4>{list.title}</h4>
        <ul aria-label={list.title}>
          {#each $view[list.kind] as rule (rule)}<li>
              <code>{rule}</code><button
                class="icon quiet"
                aria-label={`Remove ${rule}`}
                title="Remove"
                disabled={$view.saving}
                onclick={() => permissions.remove(list.kind, rule)}><Icon name="close" /></button
              >
            </li>{:else}<li class="none">No rules yet.</li>{/each}
        </ul>
        <form
          onsubmit={(event) => {
            event.preventDefault();
            void add(list.kind);
          }}
        >
          <input
            aria-label={`New ${list.kind} rule`}
            placeholder={list.kind === 'allow' ? 'command(npm test)' : 'command(rm)'}
            bind:value={drafts[list.kind]}
            disabled={$view.saving}
          /><button disabled={$view.saving || !drafts[list.kind].trim()}>Add</button>
        </form>
        <small>{list.hint}</small>
      </div>
    {/each}
  </div>
  <form
    class="wait"
    onsubmit={(event) => {
      // Enter commits the field; its change event saves once.
      event.preventDefault();
      (event.currentTarget.elements.namedItem('timeout') as HTMLInputElement | null)?.blur();
    }}
  >
    <label
      >Approval wait <span class="unit">min</span><input
        aria-label="Approval wait"
        type="number"
        min="1"
        max="240"
        step="any"
        name="timeout"
        value={$view.timeout}
        onchange={(event) => permissions.saveTimeout(event.currentTarget.value)}
        disabled={$view.saving}
      /><small>Then agy hears "no" and continues another way.</small></label
    >
  </form>
</section>

<style>
  .permissions {
    display: grid;
    gap: var(--space-3);
  }
  h3 {
    font-size: 12px;
    font-weight: 600;
  }
  h4 {
    font-size: 11px;
    font-weight: 500;
    color: var(--ink-2);
    margin-bottom: var(--space-2);
  }
  p {
    color: var(--ink-3);
    font-size: 11px;
    line-height: 1.8;
  }
  .error {
    color: var(--bad);
  }
  .feedback {
    color: var(--ok);
  }
  .lists {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-4);
  }
  .list {
    display: grid;
    gap: var(--space-2);
    align-content: start;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
    max-height: 220px;
    overflow: auto;
  }
  li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-1) var(--space-1) var(--space-3);
    border-top: 1px solid var(--rule);
    min-height: 30px;
  }
  li:first-child {
    border-top: 0;
  }
  li code {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    color: var(--ink-2);
  }
  .none {
    color: var(--ink-4);
    font-size: 11px;
  }
  form {
    display: flex;
    gap: var(--space-2);
  }
  form input {
    flex: 1;
    min-width: 0;
    font-family: var(--mono);
    font-size: 11px;
  }
  small,
  .unit {
    color: var(--ink-3);
    font-size: 10px;
    line-height: 1.6;
  }
  .wait label {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: var(--space-2);
    font-size: 11px;
    color: var(--ink-2);
    max-width: 260px;
  }
  .wait input,
  .wait small {
    grid-column: 1 / -1;
    font-family: inherit;
  }
  @media (max-width: 600px) {
    .lists {
      grid-template-columns: 1fr;
    }
  }
</style>
