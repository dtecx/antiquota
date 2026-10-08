<!-- Lists owned stack items beside read-only source comparisons and private editors. -->
<script lang="ts">
  import type { SettingsStore, SettingsSection, StackKind } from '../lib/settings';
  import type { StackItem } from '../gen/StackItem';
  let { settings, section }: { settings: SettingsStore; section: SettingsSection } = $props();
  const view = $derived(settings.state);
  const kind = $derived(
    (
      { rules: 'rule', skills: 'skill', hooks: 'hook', mcp: 'mcp', plugins: 'plugin' } as Partial<
        Record<SettingsSection, StackKind>
      >
    )[section] ?? 'rule',
  );
  const key = $derived(
    kind === 'mcp' ? 'mcps' : (`${kind}s` as 'rules' | 'skills' | 'hooks' | 'plugins'),
  );
  const rows = $derived($view.data?.[key] ?? []);
  const sources = $derived(
    ['skill', 'mcp'].includes(kind) ? (['cli', 'ide'] as const) : (['cli'] as const),
  );
  let name = $state('');
  function comparison(row: StackItem, source: 'cli' | 'ide') {
    return row[source];
  }
  async function add() {
    if (await settings.create(kind, name)) name = '';
  }
</script>

<section class="stack" aria-label="Private stack">
  <header>
    <p>
      {kind === 'rule'
        ? 'GEMINI.md is always loaded. Rule files load by their trigger.'
        : kind === 'skill'
          ? 'Skills contain SKILL.md. agy reads the body when it needs it.'
          : kind === 'hook'
            ? 'Commands on tool and turn events. Disable one to keep it without running it.'
            : kind === 'mcp'
              ? 'Tool servers agy starts for each turn, with command and args or a URL.'
              : 'Installed bundles in antiquota’s private stack.'}
    </p>
    <div class="actions">
      {#each sources as source (source)}{@const differing = rows.filter(
          (r) => comparison(r, source) === 'differs',
        )}{#if differing.length}<button
            disabled={$view.saving}
            onclick={() =>
              settings.copyAll(
                source,
                kind,
                differing.map((r) => r.name),
              )}>Copy all differing from {source === 'cli' ? 'agy CLI' : 'IDE'}</button
          >{/if}{/each}
    </div>
  </header>
  {#if kind === 'rule'}<div class="item">
      <b>GEMINI.md</b>
      <div class="actions">
        <button onclick={() => settings.edit(null, 'GEMINI.md')}>Edit GEMINI.md</button
        >{#if $view.data?.gemini_md.cli === 'differs'}<button
            disabled={$view.saving}
            onclick={() => settings.copy('cli', 'gemini_md')}>GEMINI.md from agy CLI</button
          >{/if}
      </div>
    </div>{/if}
  {#each rows as row (row.name)}<div class="item" data-name={row.name}>
      <div class="main">
        <b>{row.name}</b>
        <div class="tags">
          {#each sources as source (source)}{#if comparison(row, source)}<span
                class:warn={comparison(row, source) === 'differs'}
                >{comparison(row, source) === 'same'
                  ? 'same as'
                  : row.here
                    ? 'different from'
                    : 'only in'}
                {source === 'cli' ? 'agy CLI' : 'IDE'}</span
              >{/if}{/each}
        </div>
        {#if row.about}<small>{row.about}</small>{/if}
      </div>
      <div class="actions">
        {#if row.enabled !== undefined}<button
            role="switch"
            aria-label={`Enable ${row.name}`}
            aria-checked={row.enabled}
            disabled={$view.saving}
            onclick={() => settings.toggle(kind, row.name, !row.enabled)}
            >{row.enabled ? 'On' : 'Off'}</button
          >{/if}{#if row.here && kind !== 'plugin'}<button
            disabled={$view.loading || $view.saving}
            onclick={() => settings.edit(kind, row.name)}>Edit</button
          >{/if}{#each sources as source (source)}{#if comparison(row, source) === 'differs'}<button
              disabled={$view.saving}
              onclick={() => settings.copy(source, kind, row.name)}
              >{row.here ? 'Replace' : 'Copy'} from {source === 'cli' ? 'agy CLI' : 'IDE'}</button
            >{/if}{/each}{#if row.here}<button
            class="danger"
            disabled={$view.saving}
            onclick={() => settings.remove(kind, row.name)}>Delete</button
          >{/if}
      </div>
    </div>{:else}<p class="empty">Nothing here yet.</p>{/each}
  {#if $view.editor}<div class="editor">
      <label
        >{$view.editor.name}<textarea
          aria-label={`Edit ${$view.editor.name}`}
          rows="14"
          spellcheck="false"
          value={$view.editor.text}
          disabled={$view.editor.loading || $view.saving}
          oninput={(e) => settings.editorText(e.currentTarget.value)}></textarea></label
      >
      <div class="actions">
        <code>{$view.editor.path}</code><button
          disabled={$view.saving}
          onclick={settings.closeEditor}>Cancel editor</button
        ><button
          class="primary"
          disabled={$view.editor.loading || $view.saving}
          onclick={settings.saveEditor}>Save editor</button
        >
      </div>
    </div>{/if}
  {#if kind !== 'plugin'}<form
      onsubmit={(e) => {
        e.preventDefault();
        void add();
      }}
    >
      <input
        aria-label={`New ${kind} name`}
        placeholder={kind === 'rule' ? 'testing.md' : `${kind}-name`}
        bind:value={name}
        disabled={$view.saving}
      /><button disabled={$view.saving}>{kind === 'mcp' ? 'Add server' : `New ${kind}`}</button>
    </form>{/if}
</section>

<style>
  .stack {
    display: grid;
    gap: var(--space-4);
  }
  header {
    display: grid;
    gap: var(--space-3);
  }
  p,
  small {
    font-size: 11px;
    line-height: 1.7;
    color: var(--ink-3);
  }
  button {
    font-size: 11px;
  }
  .item {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--space-4);
    padding: var(--space-4) 0;
    border-bottom: 1px solid var(--rule);
  }
  .main {
    min-width: 0;
    display: grid;
    gap: var(--space-2);
  }
  b {
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .tags {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }
  .tags span {
    color: var(--ink-4);
    font: 10px var(--mono);
  }
  .tags .warn {
    color: var(--baton);
  }
  .actions {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
    align-items: center;
  }
  .danger {
    color: var(--bad);
  }
  .editor {
    display: grid;
    gap: var(--space-3);
    background: var(--well);
    padding: var(--space-4);
    border-radius: var(--r-sm);
  }
  label {
    font-size: 12px;
    display: grid;
    gap: var(--space-2);
  }
  textarea {
    font: 11px/1.6 var(--mono);
    min-height: 200px;
    resize: vertical;
  }
  code {
    font: 10px var(--mono);
    color: var(--ink-3);
    overflow-wrap: anywhere;
    flex: 1;
  }
  form {
    display: flex;
    gap: var(--space-2);
  }
  form input {
    flex: 1;
    min-width: 0;
  }
  @media (max-width: 760px) {
    .item {
      flex-direction: column;
    }
  }
</style>
