<!-- The bundled vector is also the source for the splash and platform icons. -->
<script lang="ts">
  import mark from '../assets/brand-mark.svg?raw';
</script>

<span class="brand-mark" aria-hidden="true">
  <!-- eslint-disable-next-line svelte/no-at-html-tags -- Static local artwork, never user input. -->
  {@html mark}
</span>

<style>
  .brand-mark {
    display: inline-flex;
    width: 30px;
    height: 30px;
    flex-shrink: 0;
  }
  .brand-mark :global(svg) {
    width: 100%;
    height: 100%;
    color: var(--ink);
  }
  .brand-mark :global(.orbit) {
    stroke: var(--baton);
  }
  .brand-mark :global(.node) {
    fill: var(--baton);
  }
</style>
