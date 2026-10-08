<!-- Owns the prompt control, attachments, dictation and send/stop; drafts live in the store. -->
<script lang="ts">
  import { notifications } from '../lib/notifications';
  import { caretPositions } from '../lib/session-view';
  import { tick, onDestroy, untrack } from 'svelte';
  import AttachmentTray from './AttachmentTray.svelte';
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import Icon from './Icon.svelte';
  import { isBusy } from '../lib/format';
  import { draftFor, goalDialog, revising } from '../lib/goals';
  import {
    mediaNotice,
    pastedName,
    readyPaths,
    removeSent,
    say,
    trays,
    uploadFiles,
  } from '../lib/media';
  import { createAutocomplete } from '../lib/autocomplete';
  import { createPromptHistory } from '../lib/prompt-history';
  import type { ComposerActions } from '../lib/composer-actions';
  import ComposerSuggestions from './ComposerSuggestions.svelte';
  import ComposerState from './ComposerState.svelte';
  import ComposerButtons from './ComposerButtons.svelte';
  import ComposerControls from './ComposerControls.svelte';
  import DictationControl from './DictationControl.svelte';
  let {
    workbench,
    api,
    connected,
    active = true,
    actions,
  }: {
    workbench: Workbench;
    api: Api;
    connected: boolean;
    active?: boolean;
    actions: ComposerActions;
  } = $props();
  const view = $derived(workbench.state);
  const running = $derived(isBusy($view.selected?.status));
  const loading = $derived($view.loading);
  const sid = $derived($view.selected?.id ?? '');
  const text = $derived($view.drafts[sid] ?? '');
  const tray = $derived($trays[sid] ?? []);
  const ready = $derived(readyPaths(tray));
  let prompt: HTMLTextAreaElement | undefined = $state();
  let picker: HTMLInputElement | undefined = $state();
  const completion = createAutocomplete({ mentionFiles: (id, q) => api.mentionFiles(id, q) }),
    suggestions = completion.state;
  const history = createPromptHistory();
  const local = $derived(
    text.trimStart().startsWith('!') ? 'shell' : text.trimStart().startsWith('#') ? 'note' : '',
  );
  const historySource = $derived(
    JSON.stringify(
      $view.events.flatMap((event) =>
        event.t === 'user' && (event.kind === 'user' || event.kind === 'message')
          ? [event.text]
          : [],
      ),
    ),
  );
  let historySid: string | null = null;
  $effect(() => {
    const id = sid;
    void historySource;
    if (loading) {
      completion.close();
      historySid = null;
      return;
    }
    if (historySid !== id) {
      historySid = id;
      untrack(() => history.select(id, $view.events));
      completion.close();
    } else untrack(() => history.sync($view.events));
  });
  onDestroy(() => completion.dispose());
  function autosize() {
    if (!prompt) return;
    prompt.style.height = 'auto';
    prompt.style.height = `${Math.min(prompt.scrollHeight, innerHeight * 0.4)}px`;
  }
  $effect(() => {
    void text;
    void tick().then(autosize);
  });
  function edit(value: string) {
    history.edited();
    workbench.draft(value);
    void completion.query(sid, value, prompt?.selectionStart ?? value.length);
  }
  function insertSuggestion() {
    const value = completion.apply();
    if (!value) return;
    const id = sid;
    history.edited();
    workbench.draft(value.text);
    void tick().then(() => {
      if (!prompt || sid !== id || !active) return;
      prompt.selectionStart = prompt.selectionEnd = value.caret;
      prompt.focus();
    });
  }
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
    const id = sid;
    await uploadFiles(api, id, files, failed);
    if (sid === id && active) prompt?.focus();
  }
  async function submit() {
    if (tray.some((file) => file.uploading)) return say('Still uploading…');
    if ((text.trim() || ready.length) && !/^[\s]*[/!#]/.test(text)) void notifications.request();
    const id = sid;
    const paths = ready;
    revising.set(false);
    const submitted = text;
    completion.close();
    if ((await actions.submit(paths)) && paths.length) removeSent(id, paths);
    if (
      sid === id &&
      submitted &&
      !submitted.trimStart().startsWith('/') &&
      !$view.error &&
      $view.drafts[id] !== submitted
    )
      history.remember(submitted);
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
    if (event.isComposing) return;
    if ($suggestions.items.length) {
      if (['ArrowUp', 'ArrowDown'].includes(event.key)) {
        event.preventDefault();
        completion.step(event.key === 'ArrowDown' ? 1 : -1);
        return;
      }
      if (['Tab', 'Enter'].includes(event.key) && !event.shiftKey) {
        event.preventDefault();
        insertSuggestion();
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        completion.close();
        return;
      }
    }
    if (
      !event.shiftKey &&
      ((event.key === 'ArrowUp' && (!text || prompt?.selectionStart === 0 || history.active())) ||
        (event.key === 'ArrowDown' && history.active()))
    ) {
      const recalled = history.step(event.key === 'ArrowUp' ? 1 : -1, text);
      if (recalled !== undefined) {
        event.preventDefault();
        const id = sid;
        workbench.draft(recalled);
        void tick().then(() => {
          if (prompt && sid === id && active)
            prompt.selectionStart = prompt.selectionEnd = recalled.length;
        });
      }
      return;
    }
    if (event.key === 'Escape' && running) {
      event.preventDefault();
      event.stopPropagation();
      void workbench.stop();
      return;
    }
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      if (connected && !$view.mutating) void submit();
    }
  }
</script>

<svelte:window onresize={autosize} />
<div class="composer-wrap">
  {#if $mediaNotice}<p class="feedback" role="status">
      {$mediaNotice}
    </p>{/if}
  <form
    class="composer"
    class:shell={local === 'shell'}
    class:note={local === 'note'}
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
      oninput={(event) => edit(event.currentTarget.value)}
      onkeydown={keydown}
      onpaste={paste}
      onselect={rememberCaret}
      onblur={() => {
        rememberCaret();
        completion.blur();
      }}
      disabled={$view.loading}
      aria-controls={`suggestions-${sid}`}
      aria-activedescendant={$suggestions.items.length
        ? `suggestion-${sid}-${$suggestions.index}`
        : undefined}></textarea>
    <ComposerSuggestions
      {sid}
      items={$suggestions.items}
      index={$suggestions.index}
      choose={(index) => {
        completion.step(index - $suggestions.index);
        insertSuggestion();
      }}
    />
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
      >
      <DictationControl {workbench} {api} {active} {sid} {text} {prompt} />
      <ComposerButtons {workbench} {running} {connected} {text} {ready} {local} {openGoal} />
    </div>
  </form>
  {#if local}<p class="local-hint">
      {local === 'shell'
        ? 'Run in the project folder; output goes to the next prompt.'
        : 'Save a note to project GEMINI.md.'}
    </p>{/if}
  <ComposerControls {workbench} changeMode={actions.changeMode} />
  <ComposerState {workbench} />
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
  .composer.shell,
  .composer.shell:focus-within {
    border-color: var(--ok);
  }
  .composer.note {
    border-color: var(--baton);
  }
  .local-hint {
    font: 10px var(--mono);
    color: var(--ink-3);
    padding-top: var(--space-2);
  }
  .composer:focus-within {
    border-color: var(--baton);
  }
  textarea {
    border: 0;
    padding: var(--space-4);
    min-height: 92px;
    max-height: 40vh;
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
  .feedback {
    margin-bottom: var(--space-2);
    color: var(--baton);
    font-size: 11px;
  }
  @media (max-width: 760px) {
    .composer-wrap {
      padding: 0 var(--space-3) var(--space-2);
    }
  }
</style>
