<!-- Owns git review for the selected session: uncommitted files, commit, worktree merge and
     removal, and the agent's recorded edits. Diffs load when a file is expanded. -->
<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import type { Api } from '../lib/api';
  import type { Workbench } from '../lib/workbench';
  import {
    type GitStatus,
    asStatus,
    canMerge,
    codeLabel,
    editHistory,
    statusLine,
  } from '../lib/git';
  import { ask } from '../lib/confirm';
  let { workbench, api }: { workbench: Workbench; api: Api } = $props();
  const view = $derived(workbench.state);
  const session = $derived($view.selected);
  const ends = $derived($view.events.filter((e) => e.t === 'turn_end').length);
  const history = $derived(editHistory($view.events));
  let status = $state<GitStatus | null>(null);
  let loaded = $state(false);
  let error = $state('');
  let message = $state('');
  let busy = $state(false);
  let diffs = $state<Record<string, string>>({});
  let historyOpened = $state.raw(new Set<string>());
  let diffRequests: Record<string, number> = {};
  let active = true,
    shown = '',
    statusGeneration = 0,
    diffGeneration = 0,
    diffRequest = 0,
    actionGeneration = 0;
  onDestroy(() => {
    active = false;
    statusGeneration++;
    diffGeneration++;
    actionGeneration++;
  });
  const current = (id: string) => active && session?.id === id;
  async function refresh(id: string) {
    const own = ++statusGeneration;
    try {
      const result = asStatus(await api.gitStatus(id));
      if (current(id) && own === statusGeneration) {
        status = result;
        error = '';
      }
    } catch (failure) {
      if (current(id) && own === statusGeneration)
        error = failure instanceof Error ? failure.message : 'Git review failed.';
    } finally {
      if (current(id) && own === statusGeneration) loaded = true;
    }
  }
  $effect(() => {
    const id = session?.id ?? '';
    void ends;
    if (shown !== id) {
      shown = id;
      statusGeneration++;
      diffGeneration++;
      actionGeneration++;
      status = null;
      loaded = false;
      error = '';
      busy = false;
      diffs = {};
      historyOpened = new Set();
      diffRequests = {};
      message = untrack(() => session?.title ?? '');
    }
    if (id) void refresh(id);
  });
  async function toggle(key: string, path: string, committed: boolean, open: boolean) {
    if (!open || key in diffs || !session) return;
    const id = session.id,
      own = diffGeneration,
      request = ++diffRequest;
    diffRequests[key] = request;
    const latest = () => current(id) && own === diffGeneration && diffRequests[key] === request;
    try {
      const result = await api.gitDiff(id, path, committed);
      if (latest()) diffs = { ...diffs, [key]: result.diff };
    } catch (failure) {
      if (latest())
        diffs = { ...diffs, [key]: failure instanceof Error ? failure.message : 'No diff.' };
    }
  }
  function expandHistory(id: string, file: string, open: boolean) {
    if (open && current(id) && !historyOpened.has(file))
      historyOpened = new Set([...historyOpened, file]);
  }
  async function act(id: string, action: () => Promise<unknown>) {
    if (!current(id) || busy) return;
    const own = ++actionGeneration;
    busy = true;
    try {
      await action();
      if (current(id) && own === actionGeneration) error = '';
    } catch (failure) {
      if (current(id) && own === actionGeneration)
        error = failure instanceof Error ? failure.message : 'The action failed.';
    } finally {
      if (current(id) && own === actionGeneration) {
        busy = false;
        diffGeneration++;
        diffs = {};
        await refresh(id);
      }
    }
  }
  async function discard(path: string, code: string) {
    if (!session || busy) return;
    const id = session.id;
    const lost = code === '??' ? 'The new file is deleted.' : 'Its uncommitted changes are lost.';
    if (!(await ask(`Discard ${path}?`, lost, { ok: 'Discard', danger: true })) || !current(id))
      return;
    await act(id, () => api.gitDiscard(id, path));
  }
  async function remove() {
    if (!session || busy) return;
    const id = session.id;
    if (
      !(await ask(
        'Remove the worktree?',
        'The worktree folder is deleted, the session moves to the main checkout and is archived. The branch stays.',
        { ok: 'Remove worktree', danger: true },
      )) ||
      !current(id)
    )
      return;
    await act(id, () => workbench.removeWorktree(false, id));
  }
</script>

<div class="changes">
  {#if !session}
    <p class="empty">Open a session to review its changes.</p>
  {:else if !loaded}
    <p class="empty">Reading git status…</p>
  {:else if !status}
    <p class="empty">This folder is not a git repository.</p>
  {:else}
    <p class="line" title={status.worktree ? status.worktree.repo : status.root}>
      {statusLine(status)}
    </p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <h3>Uncommitted</h3>
    {#if status.files.length}
      <ul aria-label="Uncommitted files">
        {#each status.files as file (`${session.id}:${file.path}`)}
          <li>
            <details
              ontoggle={(e) => toggle(`u:${file.path}`, file.path, false, e.currentTarget.open)}
            >
              <summary>
                <span class="code">{codeLabel(file.code)}</span>
                <code>{file.path}</code>
                <span class="add">+{file.add}</span><span class="del">−{file.del}</span>
              </summary>
              <pre class="diff">{diffs[`u:${file.path}`] ?? 'Loading diff…'}</pre>
              <button class="small" disabled={busy} onclick={() => discard(file.path, file.code)}
                >Discard</button
              >
            </details>
          </li>
        {/each}
      </ul>
      <form
        class="commit"
        onsubmit={(event) => {
          event.preventDefault();
          const id = session.id;
          const text = message;
          void act(id, () => api.gitCommit(id, text));
        }}
      >
        <input aria-label="Commit message" bind:value={message} placeholder="Commit message" />
        <button class="primary small" disabled={busy || !message.trim()}>Commit all</button>
      </form>
    {:else}
      <p class="quiet">No uncommitted changes.</p>
    {/if}
    {#if status.worktree}
      <h3>Committed on this branch</h3>
      {#if status.committed?.length}
        <ul aria-label="Committed files">
          {#each status.committed as file (`${session.id}:${file.path}`)}
            <li>
              <details
                ontoggle={(e) => toggle(`c:${file.path}`, file.path, true, e.currentTarget.open)}
              >
                <summary>
                  <code>{file.path}</code>
                  <span class="add">+{file.add}</span><span class="del">−{file.del}</span>
                </summary>
                <pre class="diff">{diffs[`c:${file.path}`] ?? 'Loading diff…'}</pre>
              </details>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="quiet">Nothing committed on this branch yet.</p>
      {/if}
      <div class="actions">
        <button
          class="small primary"
          disabled={busy || !canMerge(status)}
          title={`Merge into ${status.worktree.base} in the main checkout`}
          onclick={() => {
            const id = session.id;
            void act(id, () => api.gitMerge(id));
          }}>Merge into {status.worktree.base}</button
        ><button class="small" disabled={busy} onclick={remove}>Remove worktree</button>
      </div>
    {/if}
  {/if}
  {#if session}
    <h3>Agent edits</h3>
    {#if history.length}
      <ul aria-label="Agent edits">
        {#each history as entry (`${session.id}:${entry.file}`)}
          {@const id = session.id}
          <li>
            <details ontoggle={(event) => expandHistory(id, entry.file, event.currentTarget.open)}>
              <summary
                ><code>{entry.file}</code><span class="quiet"
                  >{entry.diffs.length} {entry.diffs.length === 1 ? 'edit' : 'edits'}</span
                ></summary
              >
              {#if historyOpened.has(entry.file)}
                {#each entry.diffs as diff, index (index)}<pre class="diff">{diff}</pre>{/each}
              {/if}
            </details>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="quiet">agy has not edited files in this session yet.</p>
    {/if}
  {/if}
</div>

<style>
  .changes {
    display: grid;
    gap: var(--space-3);
    align-content: start;
  }
  .empty,
  .quiet {
    color: var(--ink-3);
    font-size: 12px;
  }
  .line {
    font: 10px var(--mono);
    color: var(--ink-3);
    overflow-wrap: anywhere;
  }
  .error {
    color: var(--bad);
    font-size: 12px;
  }
  h3 {
    margin-top: var(--space-2);
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-2);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
  }
  li + li {
    border-top: 1px solid var(--rule);
  }
  summary {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2);
    cursor: pointer;
    font-size: 11px;
    list-style: none;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  summary code {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
  }
  .code {
    font: 10px var(--mono);
    color: var(--baton);
    min-width: 54px;
  }
  .add {
    color: var(--ok);
    font: 10px var(--mono);
  }
  .del {
    color: var(--bad);
    font: 10px var(--mono);
  }
  .diff {
    margin: 0 var(--space-2) var(--space-2);
    max-height: 320px;
    overflow: auto;
    padding: var(--space-2);
    border-radius: var(--r-sm);
    background: var(--well);
    font-size: 10px;
    color: var(--ink-2);
  }
  details > .small {
    margin: 0 var(--space-2) var(--space-2);
  }
  .commit {
    display: flex;
    gap: var(--space-2);
  }
  .commit input {
    flex: 1;
    min-width: 0;
    font-size: 12px;
  }
  .actions {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }
  .small {
    font-size: 11px;
    min-height: 26px;
    padding: var(--space-1) var(--space-2);
  }
</style>
