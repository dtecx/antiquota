<!-- Owns microphone lifecycle and cancels stale speech after pane changes. -->
<script lang="ts">
  import { shortcutLabel } from '../lib/commands';
  import { tick, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import { insertAt, recordingLabel, say } from '../lib/media';
  import { browserEnvironment, createDictation } from '../lib/dictation';
  import Icon from './Icon.svelte';
  let {
    workbench,
    api,
    active,
    sid,
    text,
    prompt,
  }: {
    workbench: Workbench;
    api: Api;
    active: boolean;
    sid: string;
    text: string;
    prompt: HTMLTextAreaElement | undefined;
  } = $props();
  let dictationSid = '';
  const dictation = createDictation(
    {
      draft: () => {
        dictationSid = sid;
        return text;
      },
      live: (value) => {
        if (sid === dictationSid) workbench.draft(value);
      },
      insert: (words) => {
        if (sid !== dictationSid) return;
        const start = prompt?.selectionStart ?? text.length;
        const next = insertAt(text, start, prompt?.selectionEnd ?? start, words);
        const id = sid;
        workbench.draft(next.value);
        void tick().then(() => {
          if (!prompt || sid !== id || !active) return;
          prompt.selectionStart = prompt.selectionEnd = next.caret;
          prompt.focus();
        });
      },
      transcribe: (data, type) => api.transcribe(data, type),
      notice: say,
      fail: (message) => workbench.change({ error: message }),
    },
    browserEnvironment(),
  );
  async function toggleDictation() {
    if (get(dictation.state) === 'idle') dictationSid = sid;
    await dictation.toggle();
  }
  $effect(() => {
    void sid;
    void dictation.cancel();
  });
  onDestroy(() => {
    void dictation.cancel();
  });
  const mic = dictation.state;
  const since = dictation.since;
  let clock = $state(Date.now());
  $effect(() => {
    if ($mic !== 'recording') return;
    clock = Date.now();
    const timer = setInterval(() => (clock = Date.now()), 500);
    return () => clearInterval(timer);
  });
  const micLabel = $derived(
    $mic === 'recording'
      ? recordingLabel((clock - $since) / 1000)
      : $mic === 'listening'
        ? '● listening'
        : $mic === 'transcribing'
          ? 'transcribing…'
          : '',
  );
  const micTitle = $derived(
    {
      idle: `Dictate a message (${shortcutLabel('⌘⇧M')})`,
      listening: 'Listening: click to stop',
      recording: 'Recording: click to stop and transcribe',
      transcribing: 'Transcribing…',
    }[$mic],
  );
  function keys(event: KeyboardEvent) {
    if (!active && $mic === 'idle') return;
    if (event.key === 'Escape' && ($mic === 'recording' || $mic === 'listening')) {
      event.preventDefault();
      event.stopPropagation();
      void dictation.cancel();
      return;
    }
    if (
      active &&
      (event.metaKey || event.ctrlKey) &&
      event.shiftKey &&
      event.key.toLowerCase() === 'm'
    ) {
      event.preventDefault();
      if (sid) void toggleDictation();
    }
  }
</script>

<svelte:window onkeydowncapture={keys} />
<button
  type="button"
  class="tool mic"
  class:live={$mic === 'listening' || $mic === 'recording'}
  class:busy={$mic === 'transcribing'}
  title={micTitle}
  aria-label={micTitle}
  aria-pressed={$mic === 'listening' || $mic === 'recording'}
  disabled={!sid}
  onclick={() => void toggleDictation()}><Icon name="mic" /></button
>
{#if micLabel}<span class="mic-state" role="status">{micLabel}</span>{:else}<span class="hint"
    ><kbd>↵</kbd> send <span>·</span> <kbd>⇧ ↵</kbd> newline</span
  >{/if}

<style>
  .tool {
    display: inline-grid;
    place-items: center;
    width: 32px;
    min-height: 32px;
    padding: 0;
    color: var(--ink-3);
    flex-shrink: 0;
  }
  .tool:hover:not(:disabled) {
    color: var(--ink);
  }
  .mic.live {
    color: var(--bad);
    border-color: var(--bad);
    background: var(--bad-soft);
  }
  .mic.busy {
    color: var(--baton);
    border-color: var(--baton);
  }
  .mic-state {
    margin-left: var(--space-1);
    font: 10px var(--mono);
    color: var(--bad);
    white-space: nowrap;
  }
  .hint {
    color: var(--ink-4);
    font-size: 10px;
    margin-left: var(--space-1);
  }
  @media (max-width: 760px) {
    .hint {
      display: none;
    }
  }
</style>
