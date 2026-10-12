<!-- Keeps reasoning available in both detail modes with independent expansion. -->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { EventRecord } from '../gen/EventRecord';
  import type { Expansion, TranscriptActions } from '../lib/transcript-actions';
  import { preferences } from '../lib/preferences';
  import { formatTokens } from '../lib/format';
  import Markdown from './Markdown.svelte';
  let {
    event,
    expand,
    actions,
  }: {
    event: Extract<EventRecord, { t: 'thinking' }>;
    expand: Expansion;
    actions: TranscriptActions;
  } = $props();
  const prefs = preferences.state;
  let open = $state(false),
    seen = -1,
    detail = '';
  $effect(() => {
    const intent = expand,
      next = $prefs.detail;
    if (intent.version !== seen) {
      open =
        intent.version > 0
          ? intent.open
          : next === 'full' || (next === 'verbose' && untrack(() => event.text.length < 1200));
      seen = intent.version;
    } else if (next !== detail)
      open = next === 'full' || (next === 'verbose' && untrack(() => event.text.length < 1200));
    detail = next;
  });
</script>

<details class="thinking" bind:open>
  <summary
    >Thinking {#if event.tokens}<span>{formatTokens(event.tokens)} tokens</span>{/if}</summary
  >
  <Markdown text={event.text} onfile={actions.file} oncopy={actions.copy} />
</details>

<style>
  .thinking {
    padding: var(--space-2) 0;
    font-size: 12px;
    color: var(--ink-3);
  }
  summary {
    cursor: pointer;
    min-height: 32px;
  }
  span {
    margin-left: var(--space-2);
    color: var(--ink-4);
    font-family: var(--mono);
  }
</style>
