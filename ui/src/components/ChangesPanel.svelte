<!-- Owns git review for the selected session: uncommitted files, commit, worktree merge and
     removal, and the agent's recorded edits. Diffs load when a file is expanded. -->
<script lang="ts">
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
  async function refresh(id: string) {
    try {
      status = asStatus(await api.gitStatus(id));
      error = '';
    } catch (failure) {
      error = failure instanceof Error ? failure.message : 'Git review failed.';
    } finally {
      loaded = true;
    }
  }
  $effect(() => {
    const id = session?.id;
    void ends;
    if (id) void refresh(id);
  });
  $effect(() => {
    void session?.id;
    message = session?.title ?? '';
    diffs = {};
  });
  async function toggle(key: string, path: string, committed: boolean, open: boolean) {
    if (!open || key in diffs || !session) return;
    try {
      diffs = { ...diffs, [key]: (await api.gitDiff(session.id, path, committed)).diff };
    } catch (failure) {
      diffs = { ...diffs, [key]: failure instanceof Error ? failure.message : 'No diff.' };
    }
  }
  async function act(action: () => Promise<unknown>) {
    if (!session) return;
    busy = true;
    try {
      await action();
      error = '';
    } catch (failure) {
      error = failure instanceof Error ? failure.message : 'The action failed.';
    } finally {
      busy = false;
      diffs = {};
      await refresh(session.id);
    }
  }
  async function discard(path: string, code: string) {
    const lost = code === '??' ? 'The new file is deleted.' : 'Its uncommitted changes are lost.';
    if (!session || !(await ask(`Discard ${path}?`, lost, { ok: 'Discard', danger: true }))) return;
    const id = session.id;
    await act(() => api.gitDiscard(id, path));
  }
  async function remove() {
    if (
      !session ||
      !(await ask(
        'Remove the worktree?',
        'The worktree folder is deleted, the session moves to the main checkout and is archived. The branch stays.',
        { ok: 'Remove worktree', danger: true },
      ))
    )
      return;
    await workbench.removeWorktree(false);
    await refresh(session.id);
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
        {#each status.files as file (file.path)}
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
          void act(() => api.gitCommit(id, message));
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
          {#each status.committed as file (file.path)}
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
            void act(() => api.gitMerge(id));
          }}>Merge into {status.worktree.base}</button
        ><button class="small" disabled={busy} onclick={remove}>Remove worktree</button>
      </div>
    {/if}
  {/if}
  {#if session}
    <h3>Agent edits</h3>
    {#if history.length}
      <ul aria-label="Agent edits">
        {#each history as entry (entry.file)}
          <li>
            <details>
              <summary
                ><code>{entry.file}</code><span class="quiet"
                  >{entry.diffs.length} {entry.diffs.length === 1 ? 'edit' : 'edits'}</span
                ></summary
              >
              {#each entry.diffs as diff, index (index)}<pre class="diff">{diff}</pre>{/each}
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
