<!-- Owns scroll behavior and empty/loading presentation for the event projection. -->
<script lang="ts">
  import { tick, onDestroy, untrack } from 'svelte';
  import { scrollPositions } from '../lib/session-view';
  import type { Login } from '../lib/login';
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import { groupTranscript, nearBottom } from '../lib/transcript';
  import { conversationMarkdown } from '../lib/export';
  import { createTranscriptActions, type Expansion } from '../lib/transcript-actions';
  import { earlierSessions } from '../lib/session-start';
  import { draftFor, goalDialog } from '../lib/goals';
  import { get } from 'svelte/store';
  import { preferences } from '../lib/preferences';
  import TurnGroup from './TurnGroup.svelte';
  import ConversationFind from './ConversationFind.svelte';
  import { isBusy } from '../lib/format';
  let { workbench, login, api }: { workbench: Workbench; login: Login; api: Api } = $props();
  const view = $derived(workbench.state);
  const events = $derived($view.events),
    sid = $derived($view.selected?.id ?? ''),
    loading = $derived($view.loading);
  const groups = $derived(groupTranscript(events));
  const earlier = $derived(earlierSessions($view.selected, $view.snapshot?.sessions ?? []));
  function inherit(from: string, title: string) {
    const id = get(view).selected?.id;
    if (id)
      void workbench.run(async () => {
        await api.inherit(id, from);
        workbench.change({ feedback: `Picks up from ${title || 'that session'}` });
      });
  }
  const actions = untrack(() => createTranscriptActions(api, workbench));
  const prefs = preferences.state;
  let expand = $state<Expansion>({ version: 0, open: false });
  onDestroy(actions.dispose);
  let scroller = $state.raw<HTMLDivElement>();
  let finder: { open: () => void } | undefined = $state();
  let findMatch = $state('');
  let shown = '',
    restoring = false;
  let follow = $state(true);
  async function reuse(text: string) {
    const id = get(view).selected?.id;
    if (await actions.reuse(text)) {
      await tick();
      if (get(view).selected?.id === id) document.getElementById(`prompt-${id}`)?.focus();
    }
  }
  function latest() {
    if (!scroller) return;
    follow = true;
    scroller.scrollTop = scroller.scrollHeight;
    scrolled();
    scroller.focus({ preventScroll: true });
  }
  $effect.pre(() => {
    const id = sid;
    void events;
    if (scroller && shown && id !== shown && !restoring) {
      scrollPositions.set(shown, {
        top: scroller.scrollTop,
        follow: nearBottom(scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight),
      });
      restoring = true;
    } else if (scroller && shown && !restoring && !loading) {
      follow = nearBottom(scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight);
    }
  });
  $effect(() => {
    void events;
    const id = sid;
    if (loading || !id) return;
    const changed = shown !== id,
      saved = scrollPositions.get(id);
    shown = id;
    if (changed) {
      follow = saved?.follow ?? true;
      restoring = true;
    }
    void tick().then(() => {
      if (!scroller || shown !== id || loading) return;
      if (changed) scroller.scrollTop = saved?.top ?? scroller.scrollHeight;
      else if (follow) scroller.scrollTop = scroller.scrollHeight;
      restoring = false;
    });
  });
  function scrolled() {
    if (!scroller || restoring || loading || !shown) return;
    follow = nearBottom(scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight);
    scrollPositions.set(shown, { top: scroller.scrollTop, follow });
  }
</script>

<div class="transcript-shell">
  <ConversationFind
    {groups}
    {sid}
    {scroller}
    bind:selected={findMatch}
    bind:this={finder}
    onclose={() =>
      scroller
        ?.querySelector<HTMLButtonElement>('[aria-label="Find in conversation"]')
        ?.focus({ preventScroll: true })}
  />
  <div
    class="transcript"
    bind:this={scroller}
    onscroll={scrolled}
    aria-label="Conversation"
    role="region"
    tabindex="-1"
    aria-busy={loading}
  >
    <div class="content">
      {#if groups.length}<div class="presentation" aria-label="Transcript presentation">
          <button
            class="quiet small"
            aria-label="Find in conversation"
            title="Find in this conversation (⌘F / Ctrl+F)"
            disabled={loading}
            onclick={() => finder?.open()}>Find</button
          >
          <button
            class="quiet small"
            disabled={loading}
            title="Copy prompts and completed replies as Markdown"
            onclick={() =>
              $view.selected && actions.copy(conversationMarkdown($view.selected, events))}
            >Copy conversation</button
          >
          <button
            class="quiet small"
            aria-pressed={$prefs.detail === 'detailed'}
            onclick={() =>
              preferences.change({ detail: $prefs.detail === 'detailed' ? 'compact' : 'detailed' })}
            >{$prefs.detail === 'detailed' ? 'Detailed' : 'Compact'}</button
          >
          <button
            class="quiet small"
            aria-pressed={expand.open}
            onclick={() => (expand = { version: expand.version + 1, open: !expand.open })}
            >{expand.open ? 'Collapse all' : 'Expand all'}</button
          >
        </div>{/if}
      {#if loading && !groups.length}<div class="empty">
          <span class="eyebrow">SESSION</span>
          <h2>Loading conversation…</h2>
          <p>Reading the session history.</p>
        </div>
      {:else if !groups.length}<div class="empty">
          <span class="eyebrow">READY WHEN YOU ARE</span>
          <h2>What are we working on?</h2>
          <p>
            Ask a question, inspect a project, or describe a change.<br />Each turn stays with this
            session.
          </p>
          <div class="folder">Commands run in {$view.selected?.cwd}.</div>
          <button
            onclick={() =>
              goalDialog.set(
                draftFor(
                  $view.selected?.goal ?? null,
                  $view.drafts[sid] ?? '',
                  $view.snapshot?.defaults.verifier_model || 'gemini-3.8-flash-high',
                ),
              )}>Set a goal</button
          >
          {#if earlier.length}<div class="pickup" aria-label="Earlier sessions">
              <p>Pick up from an earlier session here:</p>
              {#each earlier as previous (previous.id)}<button
                  disabled={$view.mutating}
                  title="Its handoff state goes in front of your next message"
                  onclick={() => inherit(previous.id, previous.title)}
                  >{previous.title || 'Untitled'}</button
                >{/each}
            </div>{/if}
          {#if !$view.snapshot?.accounts.length}<button onclick={() => login.open()}
              >Add a Google account</button
            >{/if}
        </div>
      {:else}{#each groups as group, index (group.key)}<TurnGroup
            {group}
            running={index === groups.length - 1 && isBusy($view.selected?.status)}
            busy={$view.mutating}
            {actions}
            {expand}
            selectedMatch={findMatch}
            onreuse={reuse}
            onlogin={() => login.open()}
            cwd={$view.selected?.cwd ?? ''}
            onanswer={workbench.answer}
            onlog={workbench.openJobLog}
            attachmentUrl={(name) => api.attachmentUrl($view.selected?.id ?? '', name)}
          />{/each}{/if}
    </div>
  </div>
  {#if !follow && groups.length && !loading}<div class="latest">
      <button onclick={latest}>Jump to latest</button>
    </div>{/if}
</div>

<style>
  .transcript-shell {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
  }
  .latest {
    position: absolute;
    bottom: var(--space-3);
    left: 0;
    right: 0;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }
  .latest button {
    pointer-events: auto;
    color: var(--ink-2);
  }
  .pickup {
    display: grid;
    gap: var(--space-2);
    margin-top: var(--space-4);
  }
  .pickup button {
    text-align: left;
  }
  .presentation {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    justify-content: flex-end;
    margin-bottom: var(--space-3);
  }
  .transcript {
    flex: 1;
    min-height: 0;
    overflow: auto;
    scroll-behavior: auto;
  }
  .content {
    max-width: var(--transcript-width);
    padding: var(--space-5) var(--space-5) 0;
    margin: 0 auto;
  }
  .empty {
    padding: min(14vh, 100px) 0 var(--space-6);
  }
  .eyebrow {
    color: var(--baton);
    font: 10px var(--mono);
    letter-spacing: 0.08em;
  }
  h2 {
    margin: var(--space-3) 0;
    font-size: 24px;
    line-height: 1.3;
    font-weight: 500;
    letter-spacing: -0.5px;
  }
  p {
    color: var(--ink-3);
    font-size: 13px;
    line-height: 1.8;
  }
  .folder {
    margin-top: var(--space-5);
    font: 11px var(--mono);
    color: var(--ink-4);
    overflow-wrap: anywhere;
  }
  @media (max-width: 760px) {
    .content {
      padding: var(--space-4) var(--space-3) 0;
    }
    h2 {
      font-size: 21px;
    }
  }
</style>
