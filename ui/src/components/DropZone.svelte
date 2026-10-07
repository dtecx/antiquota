<!-- Owns the window-wide file drop: the overlay while files hover, then the upload. -->
<script lang="ts">
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import { uploadFiles } from '../lib/media';
  let { workbench, api }: { workbench: Workbench; api: Api } = $props();
  const view = $derived(workbench.state);
  // Every element the drag crosses fires enter and leave; count them so the overlay stays.
  let depth = $state(0);
  const hasFiles = (event: DragEvent) => [...(event.dataTransfer?.types ?? [])].includes('Files');
  async function drop(files: File[]) {
    const sid = $view.selected?.id;
    if (!sid) return workbench.change({ error: 'Open or create a session first.' });
    await uploadFiles(api, sid, files, (error) =>
      workbench.change({ error: error instanceof Error ? error.message : String(error) }),
    );
    document.getElementById('prompt')?.focus();
  }
</script>

<svelte:window
  ondragenter={(event) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    depth += 1;
  }}
  ondragover={(event) => {
    if (hasFiles(event)) event.preventDefault();
  }}
  ondragleave={(event) => {
    if (hasFiles(event)) depth = Math.max(0, depth - 1);
  }}
  ondrop={(event) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    depth = 0;
    void drop([...(event.dataTransfer?.files ?? [])]);
  }}
/>
{#if depth > 0}<div class="dropzone" aria-hidden="true">
    <div>Drop files to attach</div>
  </div>{/if}

<style>
  .dropzone {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: center;
    pointer-events: none;
    background: rgb(10 11 12 / 55%);
    backdrop-filter: blur(2px);
  }
  .dropzone div {
    padding: var(--space-5) var(--space-6);
    border: 2px dashed var(--baton);
    border-radius: var(--r-lg);
    background: var(--night-1);
    color: var(--baton);
    font: 500 15px var(--sans);
  }
</style>
