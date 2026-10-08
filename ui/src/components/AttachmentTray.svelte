<!-- Renders this session's pending attachments and removal controls. -->
<script lang="ts">
  import type { Api } from '../lib/api';
  import { extensionLabel, removeFile, type PendingFile } from '../lib/media';
  import { humanSize } from '../lib/git';
  import Icon from './Icon.svelte';
  let { api, sid, files }: { api: Api; sid: string; files: PendingFile[] } = $props();
</script>

{#if files.length}<div class="tray" aria-label="Attached files">
    {#each files as file (file.key)}<div
        class="chip"
        class:uploading={file.uploading}
        title={file.name}
      >
        {#if file.image && (file.preview || file.path)}<img
            src={file.preview ?? api.attachmentUrl(sid, file.name)}
            alt=""
          />{:else}<span class="kind">{extensionLabel(file.name)}</span>{/if}<span class="name"
          >{file.name}</span
        ><span class="size">{file.uploading ? 'uploading…' : humanSize(file.size)}</span><button
          type="button"
          class="remove"
          aria-label={`Remove ${file.name}`}
          onclick={() => removeFile(sid, file)}><Icon name="close" size={12} /></button
        >
      </div>{/each}
  </div>{/if}

<style>
  .tray {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-3) 0;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    max-width: 240px;
    padding: var(--space-1) var(--space-1) var(--space-1) var(--space-1);
    border: 1px solid var(--rule-2);
    border-radius: var(--r-md);
    background: var(--well);
    font-size: 12px;
  }
  .chip.uploading {
    opacity: 0.6;
  }
  .chip img,
  .chip .kind {
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    border-radius: var(--r-sm);
  }
  .chip img {
    object-fit: cover;
    outline: 1px solid var(--rule-2);
    outline-offset: -1px;
  }
  .chip .kind {
    display: grid;
    place-items: center;
    background: var(--rule);
    color: var(--ink-3);
    font: 500 10px var(--mono);
    text-transform: uppercase;
  }
  .chip .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-2);
  }
  .chip .size {
    font: 10px var(--mono);
    color: var(--ink-4);
    white-space: nowrap;
  }
  .chip .remove {
    display: inline-grid;
    place-items: center;
    min-height: 24px;
    width: 24px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--ink-4);
  }
  .chip .remove:hover {
    color: var(--bad);
    background: var(--bad-soft);
  }
</style>
