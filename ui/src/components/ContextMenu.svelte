<!-- Owns bounded menu placement, initial focus, keyboard wrapping and outside dismissal. -->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { contextMenu, closeMenu } from '../lib/context-menu';
  let box = $state.raw<HTMLDivElement>(),
    left = $state(0),
    top = $state(0);
  onMount(() => {
    void tick().then(() => {
      const menu = $contextMenu;
      if (!box || !menu) return;
      const r = box.getBoundingClientRect();
      left = Math.max(8, Math.min(menu.x, innerWidth - r.width - 8));
      top = Math.max(8, Math.min(menu.y, innerHeight - r.height - 8));
      (box.querySelector<HTMLButtonElement>('button:not(:disabled)') ?? box).focus();
    });
  });
  function keys(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeMenu();
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      e.stopPropagation();
      const items = [...(box?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])];
      const at = items.indexOf(document.activeElement as HTMLButtonElement);
      items[(at + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length]?.focus();
    }
  }
</script>

<svelte:window
  onpointerdown={(e) => {
    if (box && !box.contains(e.target as Node)) closeMenu(false);
  }}
  onresize={() => closeMenu()}
/>
{#if $contextMenu}<div
    class="context-menu"
    role="menu"
    tabindex="-1"
    aria-label={$contextMenu.caption || 'Actions'}
    bind:this={box}
    style:left={`${left}px`}
    style:top={`${top}px`}
    onkeydown={keys}
  >
    {#if $contextMenu.caption}<p>{$contextMenu.caption}</p>{/if}
    {#each $contextMenu.items as item (item)}{#if item.heading}<p class="heading">
          {item.heading}
        </p>{/if}<button
        role={item.checked === undefined ? 'menuitem' : 'menuitemradio'}
        aria-checked={item.checked}
        class:checked={item.checked}
        class:danger={item.danger}
        disabled={item.disabled}
        onclick={() => {
          closeMenu();
          void item.action();
        }}
        >{#if item.checked !== undefined}<span class="check" aria-hidden="true"
            >{item.checked ? '✓' : ''}</span
          >{/if}{item.label}</button
      >{/each}
    {#if !$contextMenu.items.length}<p>No other session yet.</p>{/if}
  </div>{/if}

<style>
  .context-menu {
    position: fixed;
    z-index: 50;
    display: grid;
    gap: var(--space-1);
    width: max-content;
    max-width: calc(100vw - 16px);
    max-height: calc(100vh - 16px);
    overflow: auto;
    background: var(--night-2);
    border: 1px solid var(--rule-3);
    border-radius: var(--r-md);
    padding: var(--space-2);
    box-shadow: 0 12px 40px rgb(0 0 0 / 30%);
  }
  button {
    text-align: left;
    border: 0;
    background: transparent;
    font-size: 12px;
    white-space: normal;
  }
  button:hover,
  button:focus-visible {
    background: var(--baton-soft);
    color: var(--baton);
  }
  p {
    font: 10px var(--mono);
    color: var(--ink-3);
    padding: var(--space-2);
  }
  .danger {
    color: var(--bad);
  }
  .heading {
    border-top: 1px solid var(--rule);
    margin-top: var(--space-1);
    padding-top: var(--space-3);
  }
  .check {
    display: inline-block;
    width: 1.4em;
    color: var(--baton);
  }
</style>
