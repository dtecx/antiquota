<!-- Presents application-owned suggestions without owning requests or drafts. -->
<script lang="ts">
  import type { AutocompleteItem } from '../lib/autocomplete';
  let {
    sid,
    items,
    index,
    choose,
  }: { sid: string; items: AutocompleteItem[]; index: number; choose: (index: number) => void } =
    $props();
</script>

{#if items.length}<div
    class="suggestions"
    role="listbox"
    aria-label="Composer suggestions"
    id={`suggestions-${sid}`}
  >
    {#each items as item, at (item.label)}<button
        type="button"
        role="option"
        id={`suggestion-${sid}-${at}`}
        aria-selected={at === index}
        onmousedown={(e) => e.preventDefault()}
        onclick={() => choose(at)}><code>{item.label}</code><span>{item.detail}</span></button
      >{/each}
  </div>{/if}

<style>
  .suggestions {
    max-height: 220px;
    overflow: auto;
    border-top: 1px solid var(--rule);
    padding: var(--space-1);
  }
  button {
    display: flex;
    gap: var(--space-3);
    width: 100%;
    text-align: left;
    border: 0;
    font-size: 11px;
  }
  button[aria-selected='true'] {
    background: var(--baton-soft);
    color: var(--baton);
  }
  code {
    min-width: 95px;
  }
  span {
    color: var(--ink-3);
  }
</style>
