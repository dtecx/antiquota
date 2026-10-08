<!-- Owns the prompt control, attachments, dictation and send/stop; drafts live in the store. -->
<script lang="ts">
  import { notifications } from '../lib/notifications';
  import { caretPositions } from '../lib/session-view';
  import { tick, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import AttachmentTray from './AttachmentTray.svelte';
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import Icon from './Icon.svelte';
  import { formatTokens, isBusy, statusLabel } from '../lib/format';
  import { draftFor, goalDialog, revising } from '../lib/goals';
  import {
    insertAt,
    mediaNotice,
    pastedName,
    readyPaths,
    recordingLabel,
    removeSent,
    say,
    trays,
    uploadFiles,
  } from '../lib/media';
  import { browserEnvironment, createDictation } from '../lib/dictation';
  let {
    workbench,
    api,
    connected,
    active = true,
  }: { workbench: Workbench; api: Api; connected: boolean; active?: boolean } = $props();
  const view = $derived(workbench.state);
  const running = $derived(isBusy($view.selected?.status));
  const attention = $derived($view.selected?.status === 'approval');
  const loading = $derived($view.loading);
  const sid = $derived($view.selected?.id ?? '');
  const text = $derived($view.drafts[sid] ?? '');
  const tray = $derived($trays[sid] ?? []);
  const ready = $derived(readyPaths(tray));
  let prompt: HTMLTextAreaElement | undefined = $state();
  let picker: HTMLInputElement | undefined = $state();
  $effect(() => {
    const id = sid;
    if (active && !loading)
      void tick().then(() => {
        if (!prompt || sid !== id || !active || loading) return;
        const focused = document.activeElement;
        if (
          focused !== prompt &&
          focused?.closest('.session-pane') === prompt.closest('.session-pane')
        )
          return;
        const caret = caretPositions.get(id);
        if (caret) {
          prompt.selectionStart = caret.start;
          prompt.selectionEnd = caret.end;
        }
        prompt.focus();
      });
  });
  function rememberCaret() {
    if (prompt && sid)
      caretPositions.set(sid, { start: prompt.selectionStart, end: prompt.selectionEnd });
  }
  function failed(error: unknown) {
    workbench.change({ error: error instanceof Error ? error.message : String(error) });
  }
  async function attachFiles(files: File[]) {
    if (!sid) return say('Open or create a session first');
    await uploadFiles(api, sid, files, failed);
    prompt?.focus();
  }
  async function submit() {
    if (tray.some((file) => file.uploading)) return say('Still uploading…');
    void notifications.request();
    const id = sid;
    const paths = ready;
    revising.set(false);
    if ((await workbench.send(paths)) && paths.length) removeSent(id, paths);
  }
  function paste(event: ClipboardEvent) {
    const now = new Date();
    const files = [...(event.clipboardData?.files ?? [])].map((file) =>
      file.name === 'image.png' || !file.name
        ? new File([file], pastedName(file.name, now), { type: file.type })
        : file,
    );
    if (!files.length) return;
    event.preventDefault();
    void attachFiles(files);
  }
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
        workbench.draft(next.value);
        void tick().then(() => {
          if (!prompt) return;
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
      idle: 'Dictate a message (⌘⇧M)',
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
  $effect(() => {
    if ($revising && active) prompt?.focus();
  });
  function openGoal() {
    const session = $view.selected;
    if (!session) return;
    goalDialog.set(
      draftFor(session.goal, text, $view.snapshot?.defaults.verifier_model || session.model),
    );
  }
  function keydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      if (connected && !$view.mutating) void submit();
    }
  }
</script>

<svelte:window onkeydowncapture={keys} />
<div class="composer-wrap">
  {#if $mediaNotice || $view.feedback}<p class="feedback" role="status">
      {$mediaNotice || $view.feedback}
    </p>{/if}
  <form
    class="composer"
    onsubmit={(event) => {
      event.preventDefault();
      void submit();
    }}
  >
    <AttachmentTray {api} {sid} files={tray} />
    <label class="sr-only" for={`prompt-${sid}`}>Message</label><textarea
      id={`prompt-${sid}`}
      aria-label="Message"
      rows="3"
      bind:this={prompt}
      placeholder={$revising
        ? 'What should change in the plan?'
        : running
          ? 'Queue a message for the next turn…'
          : 'Message antiquota…'}
      value={text}
      oninput={(event) => workbench.draft(event.currentTarget.value)}
      onkeydown={keydown}
      onpaste={paste}
      onselect={rememberCaret}
      onblur={rememberCaret}
      disabled={!$view.selected || $view.loading}></textarea>
    <div class="composer-tools">
      <input
        class="sr-only"
        type="file"
        multiple
        tabindex="-1"
        aria-hidden="true"
        bind:this={picker}
        onchange={(event) => {
          void attachFiles([...(event.currentTarget.files ?? [])]);
          event.currentTarget.value = '';
        }}
      /><button
        type="button"
        class="tool"
        title="Add images or files (or drop them, or paste a screenshot)"
        aria-label="Add images or files"
        disabled={!$view.selected}
        onclick={() => picker?.click()}><Icon name="plus" /></button
      ><button
        type="button"
        class="tool mic"
        class:live={$mic === 'listening' || $mic === 'recording'}
        class:busy={$mic === 'transcribing'}
        title={micTitle}
        aria-label={micTitle}
        aria-pressed={$mic === 'listening' || $mic === 'recording'}
        disabled={!$view.selected}
        onclick={() => void toggleDictation()}><Icon name="mic" /></button
      >{#if micLabel}<span class="mic-state" role="status">{micLabel}</span>{:else}<span
          class="hint"><kbd>↵</kbd> send <span>·</span> <kbd>⇧ ↵</kbd> newline</span
        >{/if}
      <div class="actions">
        <button
          type="button"
          class="goal-button"
          class:active={$view.selected?.goal?.state === 'active'}
          title="Work on an objective until it is verified done"
          onclick={openGoal}
          disabled={!$view.selected}>◎ Goal</button
        >{#if running}<button
            type="button"
            class="stop"
            onclick={() => workbench.stop()}
            disabled={$view.mutating || !connected}><Icon name="stop" />Stop</button
          >{/if}<button
          type="submit"
          class="primary send"
          disabled={(!text.trim() && !ready.length) ||
            $view.mutating ||
            !connected ||
            $view.loading}><span>{running ? 'Queue' : 'Send'}</span><Icon name="send" /></button
        >
      </div>
    </div>
  </form>
  <div class="session-state">
    <span class="status" class:running class:attention
      ><span class="dot"></span>{statusLabel($view.selected?.status ?? 'idle')}</span
    >{#if $view.selected?.queue.length}<span>{$view.selected.queue.length} queued</span>{/if}<span
      class="tokens"
      >{formatTokens($view.selected?.usage.input ?? 0)} in <span>/</span>
      {formatTokens($view.selected?.usage.output ?? 0)} out</span
    >
  </div>
</div>

<style>
  .composer-wrap {
    width: 100%;
    max-width: var(--transcript-width);
    padding: 0 var(--space-5) var(--space-2);
    margin: 0 auto;
  }
  .composer {
    border: 1px solid var(--rule-3);
    border-radius: var(--r-md);
    background: var(--night-1);
    overflow: hidden;
  }
  .composer:focus-within {
    border-color: var(--baton);
  }
  textarea {
    border: 0;
    padding: var(--space-4);
    min-height: 92px;
    max-height: 30vh;
    background: transparent;
    font-size: 13px;
    line-height: 1.7;
  }
  textarea:focus {
    outline: none;
  }
  .composer-tools {
    display: flex;
    align-items: center;
    padding: 0 var(--space-3) var(--space-3);
    gap: var(--space-2);
  }
  .hint {
    color: var(--ink-4);
    font-size: 10px;
    margin-left: var(--space-1);
  }
  .tool {
    display: inline-grid;
    place-items: center;
    width: 32px;
    min-height: 32px;
    padding: 0;
    color: var(--ink-3);
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
  .actions {
    display: flex;
    gap: var(--space-2);
    margin-left: auto;
  }
  .composer-tools > button {
    flex-shrink: 0;
  }
  .send,
  .stop {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 32px;
    padding: var(--space-1) var(--space-3);
    font-size: 12px;
  }
  .stop {
    color: var(--ink-2);
  }
  .goal-button {
    min-height: 32px;
    padding: var(--space-1) var(--space-3);
    font-size: 12px;
    color: var(--ink-2);
  }
  .goal-button.active {
    color: var(--baton);
    border-color: var(--baton);
  }
  .session-state {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    min-height: 30px;
    font: 10px var(--mono);
    color: var(--ink-4);
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  .status.running {
    color: var(--baton);
  }
  .status.attention {
    color: var(--baton);
    font-weight: 600;
  }
  .status.attention .dot {
    width: 6px;
    height: 6px;
  }
  .dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: currentColor;
  }
  .tokens {
    margin-left: auto;
  }
  .tokens > span {
    color: var(--ink-4);
  }
  .feedback {
    margin-bottom: var(--space-2);
    color: var(--baton);
    font-size: 11px;
  }
  @media (max-width: 760px) {
    .composer-wrap {
      padding: 0 var(--space-3) var(--space-2);
    }
    .hint {
      display: none;
    }
  }
</style>
