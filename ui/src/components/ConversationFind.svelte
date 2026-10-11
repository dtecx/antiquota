<!-- Owns local literal message search and keyboard navigation within one transcript. -->
<script lang="ts">
  import { tick } from 'svelte';
  import { findTranscript, type TranscriptGroup } from '../lib/transcript';
  import Icon from './Icon.svelte';
  let {
    groups,
    sid,
    scroller,
    selected = $bindable(''),
    onclose,
  }: {
    groups: TranscriptGroup[];
    sid: string;
    scroller: HTMLDivElement | undefined;
    selected?: string;
    onclose: () => void;
  } = $props();
  let opened = $state(false),
    query = $state(''),
    index = $state(0);
  let input = $state.raw<HTMLInputElement>(),
    bar = $state.raw<HTMLDivElement>();
  const hits = $derived(findTranscript(groups, query));
  const position = $derived(index < hits.length ? index : 0);
  const target = $derived(opened ? (hits[position] ?? '') : '');
  export function open() {
    opened = true;
    void tick().then(() => {
      if (!opened) return;
      input?.focus();
      input?.select();
    });
  }
  function close() {
    opened = false;
    onclose();
  }
  function move(direction: number) {
    if (hits.length) index = (position + direction + hits.length) % hits.length;
  }
  function key(event: KeyboardEvent) {
    if (!opened || event.isComposing || !bar?.contains(document.activeElement)) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (event.key === 'Enter' && document.activeElement === input) {
      event.preventDefault();
      move(event.shiftKey ? -1 : 1);
    }
  }
  $effect(() => {
    void sid;
    opened = false;
    query = '';
    index = 0;
  });
  $effect(() => {
    const key = target,
      id = sid;
    if (selected !== key) selected = key;
    if (key)
      void tick().then(() => {
        if (sid !== id || target !== key) return;
        [...(scroller?.querySelectorAll<HTMLElement>('[data-find-key]') ?? [])]
          .find((node) => node.dataset.findKey === key)
          ?.scrollIntoView({ block: 'center' });
      });
  });
</script>

<svelte:window onkeydown={key} />
{#if opened}
  <div class="find" role="search" aria-label="Find in conversation" bind:this={bar}>
    <input
      type="search"
      aria-label="Find text in conversation"
      placeholder="Find in conversation…"
      bind:this={input}
      bind:value={query}
      oninput={() => (index = 0)}
    />
    <output aria-live="polite"
      >{!query.trim()
        ? 'Prompts and replies'
        : hits.length
          ? `${position + 1} of ${hits.length} messages`
          : 'No matching messages'}</output
    >
    <div class="controls">
      <button
        disabled={!hits.length}
        aria-label="Previous matching message"
        onclick={() => move(-1)}>Previous</button
      >
      <button disabled={!hits.length} aria-label="Next matching message" onclick={() => move(1)}
        >Next</button
      >
      <button class="icon quiet" aria-label="Close conversation find" onclick={close}
        ><Icon name="close" size={14} /></button
      >
    </div>
  </div>
{/if}

<style>
  .find {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-3);
    border-bottom: 1px solid var(--rule);
    background: var(--night-1);
  }
  input {
    flex: 1 1 160px;
    min-width: 0;
    font-size: 12px;
  }
  input::placeholder {
    color: var(--ink-2);
  }
  output {
    font-size: 12px;
    color: var(--ink-2);
  }
  .controls {
    display: flex;
    gap: var(--space-1);
    margin-left: auto;
  }
  button {
    min-height: 32px;
    padding: var(--space-1) var(--space-2);
    font-size: 12px;
    color: var(--ink-2);
  }
  button.icon {
    width: 32px;
  }
</style>
